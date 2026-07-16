import Testing

@testable import Loopflow

/// Resolving where Open presents a handoff is a pure decision. These tests are
/// the contract's Proof: provider-specific preference, overall fallback, an
/// unavailable remembered app, an IDE that attaches only with Claude + a known
/// session id, a remote Home, a visible fallback reason, the
/// preference-recording rule, and honest options.
///
/// The honesty invariant runs through all of it: only Ghostty (embedded), Warp
/// (command-bearing), and an IDE with Claude + a known session id (integrated
/// terminal running the exact argv) may claim `.attach`. An IDE without those
/// conditions opens the worktree without claiming to attach.
@Suite("Handoff surface resolution")
struct HandoffSurfaceTests {
    /// Everything installed with a proven local workspace. By default the
    /// provider is Claude with a known session id, so IDEs can attach. Pass
    /// `providerIsClaude: false` or `providerSessionKnown: false` to test the
    /// worktree-only fallback.
    private func fullCapability(
        workspaceProven: Bool = true,
        warpCommandBearing: Bool = true,
        isRemoteHome: Bool = false,
        installed: Set<HandoffSurface> = [.warp, .vscode, .cursor],
        providerIsClaude: Bool = true,
        providerSessionKnown: Bool = true
    ) -> HandoffSurfaceCapability {
        HandoffSurfaceCapability(
            installedApps: installed,
            workspaceProven: workspaceProven,
            warpCommandBearing: warpCommandBearing,
            isRemoteHome: isRemoteHome,
            providerIsClaude: providerIsClaude,
            providerSessionKnown: providerSessionKnown
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

    @Test("an IDE attaches with Claude + a known session id; otherwise worktree-only")
    func ideAttachDependsOnProviderAndSession() {
        let capability = fullCapability()
        // Claude with a known session id: IDEs attach — the launcher will run
        // the exact argv in the integrated terminal.
        #expect(capability.reach(.vscode) == .attach)
        #expect(capability.reach(.cursor) == .attach)
        #expect(capability.reach(.ghostty) == .attach)
        #expect(capability.reach(.warp) == .attach)

        // Without Claude, an IDE is worktree-only — no IDE terminal attach path
        // for other providers.
        let nonClaude = fullCapability(providerIsClaude: false)
        #expect(nonClaude.reach(.cursor) == .worktreeOnly)

        // Claude without a known session id: worktree-only — can't resume a
        // specific session.
        let noSession = fullCapability(providerSessionKnown: false)
        #expect(noSession.reach(.cursor) == .worktreeOnly)

        // A remembered IDE surface that can no longer attach (provider changed)
        // falls back to the embedded terminal and says why.
        var memory = HandoffSurfaceMemory()
        memory.record(.cursor, provider: "claude", home: "jack@local")
        let resolution = resolve(
            provider: "claude",
            home: "jack@local",
            memory: memory,
            capability: fullCapability(providerSessionKnown: false)
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

        // A non-Claude provider's IDE launch is worktree-only. Exercise the same
        // mutation production uses: a successful folder open returns false and
        // leaves the prior attach surface untouched.
        let recorded = memory.recordLaunch(
            .cursor,
            provider: "codex",
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
        // With Claude + a known session id, every surface attaches.
        let full = fullCapability().offeredOptions
        #expect(full.map(\.surface) == [.ghostty, .warp, .vscode, .cursor])
        #expect(full.first { $0.surface == .ghostty }?.reach == .attach)
        #expect(full.first { $0.surface == .warp }?.reach == .attach)
        let vscode = full.first { $0.surface == .vscode }
        #expect(vscode?.reach == .attach)
        #expect(vscode?.label == "VS Code")

        // Without Claude, IDEs are the weaker worktree-only action.
        let nonClaude = fullCapability(providerIsClaude: false).offeredOptions
        let nonClaudeVscode = nonClaude.first { $0.surface == .vscode }
        #expect(nonClaudeVscode?.reach == .worktreeOnly)
        #expect(nonClaudeVscode?.label == "VS Code (open worktree)")

        // Only Cursor installed, non-Claude provider, workspace proven: Ghostty
        // attaches, Cursor is the weaker worktree-only action, Warp is absent.
        let partial = fullCapability(
            installed: [.cursor],
            providerIsClaude: false
        ).offeredOptions
        #expect(partial.map(\.surface) == [.ghostty, .cursor])
        #expect(partial.first { $0.surface == .cursor }?.reach == .worktreeOnly)
    }
}
