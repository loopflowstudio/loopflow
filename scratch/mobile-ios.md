# LOO-718 native iOS implementation lane

Jack Heart invoked launch-plan to start backend and frontend work together. Continue the selected native Swift experience in this existing checkout and Session. LOO-593 owns the Machine backend independently; do not implement a competing server here.

## Deliver now

Build a real iOS target with the existing-open-Tasks list and an editable terminal for the selected existing Session, using Desktop typography/colors and the reduced key bar. Settle a viable native terminal renderer with evidence: Ghostty is preferred, not presumed iOS-ready. Verify native keyboard/IME, paste, selection, viewport resizing, hardware keys and accessibility; recorded output alone does not prove input. Use a deterministic terminal harness to verify generated input and its effects. Expose disconnected, unavailable, ended and control-loss states honestly. No fake production connectivity, inferred permission controls from terminal text, or second provider conversation.

Begin renderer/input and Task UI now using existing domain DTOs. LOO-593 owns wire schemas and shared executable fixtures; coordinate with its concrete contract as it develops. Build a small injectable transport boundary, not a new frontend Task model or generic SDK. Fixture mode is explicit. Do not invent a parallel permanent wire protocol to avoid waiting. UI and input work remain useful before launchd/remote setup is complete.

This lane can ship a buildable native client and deterministic input/rendering proof; it must not claim live mobile completion until follow-up C connects it to the service over host Tailscale. Leave remote pairing/carrier and takeover gesture unresolved rather than silently treating HTML recommendations as accepted. The selected new-Task sheet is later work after shared recoverable creation. No Waves, generic shell panes, diffs, hub, push or TTS in this slice.

## Design and coordination

Read mobile-task-brief.md, mobile-prototypes.md, prototype-tasks.html and research-native-mobile.md. mobile-task-plan.md owns later integration/cutover/creation dependencies. mobile-backend.md is now a reference snapshot; the working backend design is in LOO-593 at /Users/jack/src/loopflow.loo-593-decide-the-desktop/scratch/mobile-backend.md. Read that artifact for current contract decisions and use actual shared DTO/fixture code once available.

Preserve #1807 / LOO-582 Session identity, #1809 / LOO-720 terminal status and #1791 / LOO-673 Task-list behavior. #1808 changes shared Wave/RegistryQuery types; use the landed shape. Backend installation follows #1656 / LOO-312; Graph/agent defaults follow #1805 / LOO-714. UI automation stays app-owned; retention/snapshot/transport policy stays with existing execution owners. Do not modify those policies to make a fixture pass.

LOO-718 owns iOS targets, renderer and native input/UI. LOO-593 owns Rust protocol/shared fixtures and representative macOS integration. Coordinate shared Swift package/project changes explicitly; do not duplicate DTOs in two packages. Keep this existing worktree and primary conversation. The feature Graph begins at implement and stops at demo; no Auto-Ship approval.
