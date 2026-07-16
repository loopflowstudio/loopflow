import Testing

@testable import Loopflow

/// Resolving where Open presents a handoff is a pure decision. These tests are
/// the contract's Proof: provider-specific preference, overall fallback, an
/// unavailable remembered app, an IDE that never claims attach, a remote Home, a
/// visible fallback reason, the preference-recording rule, and honest options.
///
/// The honesty invariant runs through all of it: only Ghostty (embedded) and
/// Warp (command-bearing) run the exact shared attach command, so only they may
/// be honored as an attach preference. An IDE opens the worktree without ever
/// claiming to attach.
@Suite("Handoff surface resolution")
struct HandoffSurfaceTests {
    /// Everything installed with a proven local workspace. Ghostty and Warp
    /// attach; IDEs are offered only as the weaker worktree-only action.
    private func fullCapability(
        workspaceProven: Bool = true,
        warpCommandBearing: Bool = true,
        isRemoteHome: Bool = false,
        installed: Set<HandoffSurface> = [.warp, .vscode, .cursor]
    ) -> HandoffSurfaceCapability {
        HandoffSurfaceCapability(
            installedApps: installed,
            workspaceProven: workspaceProven,
            warpCommandBearing: warpCommandBearing,
            isRemoteHome: isRemoteHome
        )
    }

    private func resolve(
        provider: String,
        home: String,
        memory: HandoffSurfaceMemory,
        capability: HandoffSurfaceCapability
    ) -> HandoffSurfaceResolution {
        HandoffSurfaceResolver.resolve(
            provider: provider,
            home: home,
            memory: memory,
            capability: capability
        )
    }

    @Test("an IDE never attaches — it opens the worktree without claiming to")
    func ideNeverAttaches() {
        let capability = fullCapability()
        #expect(capability.reach(.vscode) == .worktreeOnly)
        #expect(capability.reach(.cursor) == .worktreeOnly)
        #expect(capability.reach(.ghostty) == .attach)
        #expect(capability.reach(.warp) == .attach)

        // A remembered IDE surface is never honored as an attach preference; Open
        // falls back to the embedded terminal and says why.
        var memory = HandoffSurfaceMemory()
        memory.record(.cursor, provider: "claude", home: "jack@local")
        let resolution = resolve(
            provider: "claude",
            home: "jack@local",
            memory: memory,
            capability: capability
        )
        #expect(resolution.surface == .ghostty)
        #expect(resolution.fallbackReason == "Cursor can no longer attach — using the embedded terminal.")
    }

    @Test("provider-on-Home preference wins over the overall preference")
    func providerSpecificPreference() {
        var memory = HandoffSurfaceMemory()
        memory.record(.ghostty, provider: "codex", home: "jack@local")
        memory.record(.warp, provider: "claude", home: "jack@local")

        let resolution = resolve(
            provider: "claude",
            home: "jack@local",
            memory: memory,
            capability: fullCapability()
        )
        #expect(resolution.surface == .warp)
        #expect(resolution.fallbackReason == nil)
    }

    @Test("with no provider-on-Home memory, the overall preference is used")
    func overallFallback() {
        var memory = HandoffSurfaceMemory()
        memory.record(.warp, provider: "codex", home: "jack@remote")

        let resolution = resolve(
            provider: "claude",
            home: "jack@local",
            memory: memory,
            capability: fullCapability()
        )
        #expect(resolution.surface == .warp)
    }

    @Test("nothing remembered yet is the plain Ghostty default, not a fallback")
    func plainDefaultHasNoReason() {
        let resolution = resolve(
            provider: "claude",
            home: "jack@local",
            memory: HandoffSurfaceMemory(),
            capability: fullCapability()
        )
        #expect(resolution.surface == .ghostty)
        #expect(resolution.fallbackReason == nil)
    }

    @Test("a remembered app that is no longer installed falls back with its reason")
    func unavailableRememberedApp() {
        var memory = HandoffSurfaceMemory()
        memory.record(.warp, provider: "claude", home: "jack@local")

        let resolution = resolve(
            provider: "claude",
            home: "jack@local",
            memory: memory,
            capability: fullCapability(installed: [.vscode, .cursor])
        )
        #expect(resolution.surface == .ghostty)
        #expect(resolution.fallbackReason == "Warp is unavailable — using the embedded terminal.")
    }

    @Test("a remembered surface whose capability was lost falls back with its reason")
    func capabilityLossFallsBack() {
        var memory = HandoffSurfaceMemory()
        memory.record(.warp, provider: "claude", home: "jack@local")

        // Warp stays installed but can no longer be handed a command-bearing
        // config, so it drops to worktree-only and is not honored as attach.
        let resolution = resolve(
            provider: "claude",
            home: "jack@local",
            memory: memory,
            capability: fullCapability(warpCommandBearing: false)
        )
        #expect(resolution.surface == .ghostty)
        #expect(resolution.fallbackReason == "Warp can no longer attach — using the embedded terminal.")
    }

    @Test("on a remote Home, IDEs and plain windows are unavailable; attach still works")
    func remoteHomeShapesReach() {
        // Command-bearing Warp and embedded Ghostty run the shared argv, which
        // carries its own transport, so they attach across a remote Home.
        let remote = fullCapability(isRemoteHome: true)
        #expect(remote.reach(.ghostty) == .attach)
        #expect(remote.reach(.warp) == .attach)
        // A local editor cannot open a remote worktree.
        #expect(remote.reach(.vscode) == .unavailable)
        #expect(remote.reach(.cursor) == .unavailable)

        // Without a command-bearing config, a plain Warp window would only reach a
        // local path, so on a remote Home it is unavailable rather than worktree-only.
        let remoteNoConfig = fullCapability(warpCommandBearing: false, isRemoteHome: true)
        #expect(remoteNoConfig.reach(.warp) == .unavailable)
        #expect(remoteNoConfig.offeredOptions.map(\.surface) == [.ghostty])

        // A remembered Warp on a remote Home is still honored — it attaches.
        var memory = HandoffSurfaceMemory()
        memory.record(.warp, provider: "claude", home: "ssh://jack@remote")
        let resolution = resolve(
            provider: "claude",
            home: "ssh://jack@remote",
            memory: memory,
            capability: remote
        )
        #expect(resolution.surface == .warp)
        #expect(resolution.fallbackReason == nil)
    }

    @Test("preferences are keyed by Home, so a remote Home keeps its own memory")
    func remoteHomeKeying() {
        var memory = HandoffSurfaceMemory()
        memory.record(.warp, provider: "claude", home: "ssh://jack@remote")

        #expect(memory.preferred(provider: "claude", home: "jack@local") == nil)
        #expect(memory.preferred(provider: "claude", home: "ssh://jack@remote") == .warp)
    }

    @Test("only a user-initiated, successful attach launch records a preference")
    func preferenceRecordingRule() {
        var memory = HandoffSurfaceMemory()
        let recordedAttach = memory.recordLaunch(
            .warp,
            provider: "claude",
            home: "jack@local",
            reach: .attach,
            userInitiated: true,
            launchSucceeded: true
        )
        #expect(recordedAttach)
        #expect(memory.preferred(provider: "claude", home: "jack@local") == .warp)

        // Failed and automatically-resolved launches do not replace that success.
        let recordedAutomatic = memory.recordLaunch(
            .ghostty,
            provider: "claude",
            home: "jack@local",
            reach: .attach,
            userInitiated: false,
            launchSucceeded: true
        )
        let recordedFailure = memory.recordLaunch(
            .ghostty,
            provider: "claude",
            home: "jack@local",
            reach: .attach,
            userInitiated: true,
            launchSucceeded: false
        )
        #expect(!recordedAutomatic)
        #expect(!recordedFailure)
        #expect(memory.preferred(provider: "claude", home: "jack@local") == .warp)
    }

    @Test("a worktree-only IDE success leaves the remembered attach surface intact")
    func worktreeOnlyDoesNotOverwriteAttachPreference() {
        var memory = HandoffSurfaceMemory()
        memory.record(.warp, provider: "claude", home: "jack@local")

        // Exercise the same mutation production uses. A successful folder open
        // returns false and leaves the prior attach surface untouched.
        let recorded = memory.recordLaunch(
            .cursor,
            provider: "claude",
            home: "jack@local",
            reach: .worktreeOnly,
            userInitiated: true,
            launchSucceeded: true
        )
        #expect(!recorded)
        #expect(memory.preferred(provider: "claude", home: "jack@local") == .warp)
    }

    @Test("recording an attach success advances both scopes of the preference")
    func recordAdvancesPreferenceOnSuccess() {
        var memory = HandoffSurfaceMemory()
        memory.record(.ghostty, provider: "claude", home: "jack@local")

        let recorded = memory.recordLaunch(
            .warp,
            provider: "claude",
            home: "jack@local",
            reach: .attach,
            userInitiated: true,
            launchSucceeded: true
        )
        #expect(recorded)
        #expect(memory.preferred(provider: "claude", home: "jack@local") == .warp)
        #expect(memory.overallPreferred == .warp)
    }

    @Test("offered options are honest and Ghostty always leads")
    func offeredOptionsAreHonest() {
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
