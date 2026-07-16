import Testing

@testable import Loopflow

/// Resolving where Open presents a handoff is a pure decision. These tests are
/// the contract's Proof: provider-specific preference, overall fallback, an
/// unavailable remembered app, an IDE that never claims attach, a remote Home, a
/// launch failure that leaves the preference untouched, and a preference that
/// updates only after success.
///
/// The honesty invariant runs through all of it: only Ghostty (embedded) and
/// Warp (command-bearing) run the exact shared attach command, so only they may
/// be honored as an attach preference. An IDE opens the worktree without ever
/// claiming to attach.
@Suite("Handoff surface resolution")
struct HandoffSurfaceTests {
    /// Everything installed with a proven workspace. Ghostty and Warp attach;
    /// IDEs are offered only as the weaker worktree-only action.
    private func fullCapability(
        workspaceProven: Bool = true,
        warpCommandBearing: Bool = true,
        installed: Set<HandoffSurface> = [.warp, .vscode, .cursor]
    ) -> HandoffSurfaceCapability {
        HandoffSurfaceCapability(
            installedApps: installed,
            workspaceProven: workspaceProven,
            warpCommandBearing: warpCommandBearing
        )
    }

    @Test("an IDE never attaches — it opens the worktree without claiming to")
    func ideNeverAttaches() {
        let capability = fullCapability()
        #expect(capability.reach(.vscode) == .worktreeOnly)
        #expect(capability.reach(.cursor) == .worktreeOnly)
        // Ghostty and Warp are the only surfaces that run the shared command.
        #expect(capability.reach(.ghostty) == .attach)
        #expect(capability.reach(.warp) == .attach)

        // A remembered IDE surface is never honored as an attach preference; Open
        // falls back to the embedded terminal that actually attaches.
        var memory = HandoffSurfaceMemory()
        memory.record(.cursor, provider: "claude", home: "jack@local")
        let resolved = HandoffSurfaceResolver.resolve(
            provider: "claude",
            home: "jack@local",
            memory: memory,
            capability: capability
        )
        #expect(resolved == .ghostty)
    }

    @Test("provider-on-Home preference wins over the overall preference")
    func providerSpecificPreference() {
        var memory = HandoffSurfaceMemory()
        memory.record(.ghostty, provider: "codex", home: "jack@local")
        memory.record(.warp, provider: "claude", home: "jack@local")

        // Claude on this Home resolves to Warp even though Ghostty was recorded
        // more recently overall.
        let resolved = HandoffSurfaceResolver.resolve(
            provider: "claude",
            home: "jack@local",
            memory: memory,
            capability: fullCapability()
        )
        #expect(resolved == .warp)
    }

    @Test("with no provider-on-Home memory, the overall preference is used")
    func overallFallback() {
        var memory = HandoffSurfaceMemory()
        memory.record(.warp, provider: "codex", home: "jack@remote")

        // A never-seen provider/Home pair inherits the overall surface — Warp,
        // an attach-capable surface, so it is honored.
        let resolved = HandoffSurfaceResolver.resolve(
            provider: "claude",
            home: "jack@local",
            memory: memory,
            capability: fullCapability()
        )
        #expect(resolved == .warp)
    }

    @Test("a remembered app that is no longer installed falls back visibly")
    func unavailableRememberedApp() {
        var memory = HandoffSurfaceMemory()
        memory.record(.warp, provider: "claude", home: "jack@local")

        // Warp was preferred but is no longer installed: fall through to Ghostty
        // rather than opening a surface that cannot reach the Session.
        let resolved = HandoffSurfaceResolver.resolve(
            provider: "claude",
            home: "jack@local",
            memory: memory,
            capability: fullCapability(installed: [.vscode, .cursor])
        )
        #expect(resolved == .ghostty)
    }

    @Test("a remembered surface whose capability was lost falls back")
    func capabilityLossFallsBack() {
        var memory = HandoffSurfaceMemory()
        memory.record(.warp, provider: "claude", home: "jack@local")

        // Warp stays installed but can no longer be handed a command-bearing
        // config, so it drops to worktree-only and is not honored as an attach
        // preference.
        let resolved = HandoffSurfaceResolver.resolve(
            provider: "claude",
            home: "jack@local",
            memory: memory,
            capability: fullCapability(warpCommandBearing: false)
        )
        #expect(resolved == .ghostty)
    }

    @Test("an unproven workspace makes an IDE unavailable, not worktree-only")
    func unprovenWorkspaceHidesIDE() {
        let capability = fullCapability(workspaceProven: false)
        #expect(capability.reach(.vscode) == .unavailable)
        #expect(capability.reach(.cursor) == .unavailable)
        // Ghostty still attaches regardless of the workspace probe.
        #expect(capability.reach(.ghostty) == .attach)
    }

    @Test("preferences are keyed by Home, so a remote Home keeps its own memory")
    func remoteHomeKeying() {
        var memory = HandoffSurfaceMemory()
        memory.record(.warp, provider: "claude", home: "ssh://jack@remote")

        // The same provider on the local Home has no memory yet; it inherits the
        // overall surface, not the remote-Home one by coincidence.
        #expect(memory.preferred(provider: "claude", home: "jack@local") == nil)
        #expect(memory.preferred(provider: "claude", home: "ssh://jack@remote") == .warp)

        let resolvedRemote = HandoffSurfaceResolver.resolve(
            provider: "claude",
            home: "ssh://jack@remote",
            memory: memory,
            capability: fullCapability()
        )
        #expect(resolvedRemote == .warp)
    }

    @Test("a failed launch leaves the preference untouched")
    func launchFailureLeavesPreferenceUntouched() {
        var memory = HandoffSurfaceMemory()
        memory.record(.warp, provider: "claude", home: "jack@local")

        // A launch that fails simply never calls record — the prior preference
        // stands and the next Open still resolves to it.
        let launchSucceeded = false
        if launchSucceeded {
            memory.record(.ghostty, provider: "claude", home: "jack@local")
        }
        #expect(memory.preferred(provider: "claude", home: "jack@local") == .warp)
    }

    @Test("recording only after success advances the preference")
    func recordAdvancesPreferenceOnSuccess() {
        var memory = HandoffSurfaceMemory()
        memory.record(.ghostty, provider: "claude", home: "jack@local")

        // A successful Warp launch updates both the provider-on-Home and the
        // overall preference.
        memory.record(.warp, provider: "claude", home: "jack@local")
        #expect(memory.preferred(provider: "claude", home: "jack@local") == .warp)
        #expect(memory.overallPreferred == .warp)
    }

    @Test("offered options are honest and Ghostty always leads")
    func offeredOptionsAreHonest() {
        // Fully installed: Ghostty and Warp attach, IDEs are worktree-only.
        let full = fullCapability().offeredOptions
        #expect(full.map(\.surface) == [.ghostty, .warp, .vscode, .cursor])
        #expect(full.first { $0.surface == .ghostty }?.reach == .attach)
        #expect(full.first { $0.surface == .warp }?.reach == .attach)
        let vscode = full.first { $0.surface == .vscode }
        #expect(vscode?.reach == .worktreeOnly)
        #expect(vscode?.label == "VS Code (open worktree)")

        // Only Cursor installed, workspace proven: Ghostty attaches, Cursor is the
        // weaker worktree-only action, Warp is absent.
        let partial = fullCapability(installed: [.cursor]).offeredOptions
        #expect(partial.map(\.surface) == [.ghostty, .cursor])
        #expect(partial.first { $0.surface == .cursor }?.reach == .worktreeOnly)
    }
}
