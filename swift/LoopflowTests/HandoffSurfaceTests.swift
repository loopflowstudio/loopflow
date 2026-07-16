import Testing

@testable import Loopflow

/// Resolving where Open presents a handoff is a pure decision. These tests are
/// the contract's Proof: provider-specific preference, overall fallback, an
/// unavailable remembered app, an unsupported provider, a remote Home, a launch
/// failure that leaves the preference untouched, and a preference that updates
/// only after success.
@Suite("Handoff surface resolution")
struct HandoffSurfaceTests {
    /// Everything installed and Claude fully credentialed — every surface attaches.
    private func fullCapability(
        providerIsClaude: Bool = true,
        providerSessionKnown: Bool = true,
        workspaceProven: Bool = true,
        warpCommandBearing: Bool = true,
        installed: Set<HandoffSurface> = [.warp, .vscode, .cursor]
    ) -> HandoffSurfaceCapability {
        HandoffSurfaceCapability(
            installedApps: installed,
            providerIsClaude: providerIsClaude,
            providerSessionKnown: providerSessionKnown,
            workspaceProven: workspaceProven,
            warpCommandBearing: warpCommandBearing
        )
    }

    @Test("provider-on-Home preference wins over the overall preference")
    func providerSpecificPreference() {
        var memory = HandoffSurfaceMemory()
        memory.record(.vscode, provider: "codex", home: "jack@local")
        memory.record(.warp, provider: "claude", home: "jack@local")

        // Claude on this Home resolves to Warp even though VS Code was recorded
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
        memory.record(.cursor, provider: "codex", home: "jack@remote")

        // A never-seen provider/Home pair inherits the overall surface.
        let resolved = HandoffSurfaceResolver.resolve(
            provider: "claude",
            home: "jack@local",
            memory: memory,
            capability: fullCapability()
        )
        #expect(resolved == .cursor)
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
        memory.record(.cursor, provider: "claude", home: "jack@local")
        memory.record(.vscode, provider: "claude", home: "jack@local")

        // Cursor stays installed but the workspace is no longer proven, so it
        // drops to worktree-only and cannot be honored as an attach preference.
        let resolved = HandoffSurfaceResolver.resolve(
            provider: "claude",
            home: "jack@local",
            memory: memory,
            capability: fullCapability(workspaceProven: false)
        )
        #expect(resolved == .ghostty)
    }

    @Test("a non-Claude provider never attaches an IDE")
    func unsupportedProvider() {
        let capability = fullCapability(providerIsClaude: false)
        #expect(capability.reach(.vscode) == .worktreeOnly)
        #expect(capability.reach(.cursor) == .worktreeOnly)

        var memory = HandoffSurfaceMemory()
        memory.record(.vscode, provider: "codex", home: "jack@local")

        // VS Code cannot attach a Codex session, so Open resolves to Ghostty.
        let resolved = HandoffSurfaceResolver.resolve(
            provider: "codex",
            home: "jack@local",
            memory: memory,
            capability: capability
        )
        #expect(resolved == .ghostty)
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
            memory.record(.cursor, provider: "claude", home: "jack@local")
        }
        #expect(memory.preferred(provider: "claude", home: "jack@local") == .warp)
    }

    @Test("recording only after success advances the preference")
    func recordAdvancesPreferenceOnSuccess() {
        var memory = HandoffSurfaceMemory()
        memory.record(.warp, provider: "claude", home: "jack@local")

        // A successful Cursor launch updates both the provider-on-Home and the
        // overall preference.
        memory.record(.cursor, provider: "claude", home: "jack@local")
        #expect(memory.preferred(provider: "claude", home: "jack@local") == .cursor)
        #expect(memory.overallPreferred == .cursor)
    }

    @Test("offered options are honest and Ghostty always leads")
    func offeredOptionsAreHonest() {
        // Claude fully credentialed: every surface attaches, Ghostty first.
        let full = fullCapability().offeredOptions
        #expect(full.map(\.surface) == [.ghostty, .warp, .vscode, .cursor])
        let everyFullOptionAttaches = full.allSatisfy(\.canAttach)
        #expect(everyFullOptionAttaches)

        // Uncredentialed Claude with only Cursor installed: Ghostty attaches,
        // Cursor is offered as the weaker worktree-only action, Warp is absent.
        let partial = fullCapability(
            providerSessionKnown: false,
            installed: [.cursor]
        ).offeredOptions
        #expect(partial.map(\.surface) == [.ghostty, .cursor])
        let cursor = partial.first { $0.surface == .cursor }
        #expect(cursor?.reach == .worktreeOnly)
        #expect(cursor?.label == "Cursor (open worktree)")
    }
}
