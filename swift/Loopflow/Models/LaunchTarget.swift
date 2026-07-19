import Foundation

/// A place to present an interactive launch. Every surface can open the
/// worktree; only some can honestly *attach* the exact Launch by
/// running the shared attach command against it.
public enum LaunchTarget: String, Codable, Sendable, Hashable, CaseIterable {
    /// Embedded terminal. The required local fallback: always available, always
    /// runs the exact shared attach command.
    case ghostty
    /// External terminal. Attaches through a command-bearing launch configuration.
    case warp
    /// Claude in VS Code.
    case vscode
    /// Claude in Cursor.
    case cursor

    /// An editor target, not a terminal. IDE attach names Claude specifically.
    public var isIDE: Bool { self == .vscode || self == .cursor }

    /// Human-facing name of the app.
    public var appName: String {
        switch self {
        case .ghostty: "Ghostty"
        case .warp: "Warp"
        case .vscode: "VS Code"
        case .cursor: "Cursor"
        }
    }
}

/// How honestly a surface reaches a launch's provider session right now.
public enum LaunchTargetReach: String, Codable, Sendable, Hashable {
    /// Runs the exact shared attach command against the same provider session.
    case attach
    /// Opens the worktree only — a weaker action that never claims to attach.
    case worktreeOnly
    /// The app is not installed, or the surface cannot apply to this launch.
    case unavailable
}

/// One option the surface picker offers, with the honest reach it would deliver.
/// A `worktreeOnly` reach is presented as the weaker action it is.
public struct LaunchTargetOption: Codable, Sendable, Hashable, Identifiable {
    public let surface: LaunchTarget
    public let reach: LaunchTargetReach

    public init(surface: LaunchTarget, reach: LaunchTargetReach) {
        self.surface = surface
        self.reach = reach
    }

    public var id: LaunchTarget { surface }

    public var canAttach: Bool { reach == .attach }

    /// Menu label that never overclaims: attach vs. the weaker "open worktree".
    public var label: String {
        switch reach {
        case .attach: surface.appName
        case .worktreeOnly: "\(surface.appName) (open worktree)"
        case .unavailable: surface.appName
        }
    }
}

/// What the current machine and launch permit. The resolver reads only these
/// facts; it never touches the filesystem or `NSWorkspace` itself, which keeps
/// the decision pure and testable.
///
/// Only a surface that runs the *exact shared attach command* may claim
/// `.attach`: Ghostty embeds it, Warp runs it through a command-bearing launch
/// configuration, and an IDE opens its integrated terminal and runs the command
/// there. An IDE can only do this when the provider is Claude and a specific
/// provider session id is known — without those, the IDE opens the worktree
/// without claiming to attach.
///
/// The descriptor's Home matters: on a **remote** Home the worktree lives on
/// another host, so a local editor or a plain local window cannot reach it.
/// Ghostty and a command-bearing Warp still attach — the launcher transports the
/// shared argv to the descriptor's host — but local worktree-only actions become
/// unavailable rather than opening a path that is not there.
public struct LaunchTargetCapability: Sendable, Hashable {
    /// External apps installed on the current Home. Ghostty is embedded, so it is
    /// never listed here and is always available.
    public let installedApps: Set<LaunchTarget>
    /// The worktree is a proven workspace an IDE can open (only meaningful for a
    /// local Home; a remote worktree is never locally proven).
    public let workspaceProven: Bool
    /// Warp can be handed a command-bearing launch configuration that runs the
    /// exact shared attach command.
    public let warpCommandBearing: Bool
    /// The launch's Home is on another host (`descriptor.host`), so the worktree
    /// is not on this machine.
    public let isRemoteHome: Bool
    /// The launch's provider is Claude, which can resume a specific session in
    /// an IDE's integrated terminal. Other providers have no IDE attach path.
    public let providerIsClaude: Bool
    /// A specific provider session id is known, so the IDE terminal can run
    /// `claude --resume <id>` rather than starting a fresh session.
    public let providerSessionKnown: Bool

    public init(
        installedApps: Set<LaunchTarget>,
        workspaceProven: Bool,
        warpCommandBearing: Bool,
        isRemoteHome: Bool,
        providerIsClaude: Bool = false,
        providerSessionKnown: Bool = false
    ) {
        self.installedApps = installedApps
        self.workspaceProven = workspaceProven
        self.warpCommandBearing = warpCommandBearing
        self.isRemoteHome = isRemoteHome
        self.providerIsClaude = providerIsClaude
        self.providerSessionKnown = providerSessionKnown
    }

    /// Whether an IDE can attach this launch — Claude with a known session id
    /// on a local, proven workspace. The launcher will open the IDE's integrated
    /// terminal and run the exact shared attach command.
    private var ideCanAttach: Bool {
        providerIsClaude && providerSessionKnown
    }

    /// How honestly `surface` can present this launch right now.
    public func reach(_ surface: LaunchTarget) -> LaunchTargetReach {
        switch surface {
        case .ghostty:
            // Embedded and required: the launcher runs the shared argv locally or
            // transports it to the descriptor host, so it attaches on either Home.
            return .attach
        case .warp:
            guard installedApps.contains(.warp) else { return .unavailable }
            // A command-bearing config runs the shared argv, so it attaches on any
            // Home. A plain worktree window only reaches a *local* path, so on a
            // remote Home it is unavailable rather than worktree-only.
            if warpCommandBearing { return .attach }
            return isRemoteHome ? .unavailable : .worktreeOnly
        case .vscode, .cursor:
            // A local editor cannot open a remote worktree.
            guard !isRemoteHome, installedApps.contains(surface), workspaceProven else {
                return .unavailable
            }
            // Claude with a known session id can resume the exact Launch
            // in the IDE's integrated terminal. Without those, the IDE can only
            // open the worktree — a weaker action labeled as such.
            return ideCanAttach ? .attach : .worktreeOnly
        }
    }

    /// The honest picker: every applicable surface, each labeled by its reach,
    /// in a stable order with the required Ghostty fallback first.
    public var offeredOptions: [LaunchTargetOption] {
        LaunchTarget.allCases.compactMap { surface in
            let reach = reach(surface)
            guard reach != .unavailable else { return nil }
            return LaunchTargetOption(surface: surface, reach: reach)
        }
    }
}

/// The last surface that successfully presented a launch, remembered per
/// `(provider, Home)` and once overall. Recorded only after a launch succeeds,
/// so a failed attempt never rewrites the preference.
public struct LaunchTargetMemory: Codable, Sendable, Hashable {
    private var byProviderHome: [String: LaunchTarget]
    private var overall: LaunchTarget?

    public init(byProviderHome: [String: LaunchTarget] = [:], overall: LaunchTarget? = nil) {
        self.byProviderHome = byProviderHome
        self.overall = overall
    }

    // A Home address never contains a unit-separator, so it is a safe join byte.
    private static func key(provider: String, home: String) -> String {
        "\(provider)\u{1f}\(home)"
    }

    /// The surface remembered for this provider on this Home, if any.
    public func preferred(provider: String, home: String) -> LaunchTarget? {
        byProviderHome[Self.key(provider: provider, home: home)]
    }

    /// The surface remembered across every provider and Home, if any.
    public var overallPreferred: LaunchTarget? { overall }

    /// Record a surface as the new preference after its launch succeeded.
    public mutating func record(_ surface: LaunchTarget, provider: String, home: String) {
        byProviderHome[Self.key(provider: provider, home: home)] = surface
        overall = surface
    }

    /// Record a launch only when it earned the right to become Open's default.
    /// Folder-only IDE opens, automatic resolution, and failed launches leave the
    /// last attach-capable preference untouched.
    @discardableResult
    public mutating func recordLaunch(
        _ surface: LaunchTarget,
        provider: String,
        home: String,
        reach: LaunchTargetReach,
        userInitiated: Bool,
        launchSucceeded: Bool
    ) -> Bool {
        guard userInitiated, launchSucceeded, reach == .attach else { return false }
        record(surface, provider: provider, home: home)
        return true
    }
}

/// The outcome of resolving Open's default surface: the chosen surface plus, when
/// a remembered surface could not be honored, a human-readable reason naming it
/// and why it was skipped. The view surfaces that reason so a fallback is never
/// silent.
public struct LaunchTargetResolution: Sendable, Hashable {
    public let surface: LaunchTarget
    public let fallbackReason: String?

    public init(surface: LaunchTarget, fallbackReason: String?) {
        self.surface = surface
        self.fallbackReason = fallbackReason
    }
}

/// Decides which surface Open uses by default. The decision is pure: the
/// remembered surface for this provider on this Home, then the remembered
/// overall surface, then the embedded Ghostty fallback. A remembered surface is
/// honored only while it can still `.attach`, so an uninstalled app or a lost
/// capability falls back visibly to the next candidate rather than opening a
/// surface that would lie about reaching the Session — and the resolution
/// reports *why* it fell back so the reason can be shown.
public enum LaunchTargetResolver {
    public static func resolve(
        provider: String,
        home: String,
        memory: LaunchTargetMemory,
        capability: LaunchTargetCapability
    ) -> LaunchTargetResolution {
        guard let remembered = memory.preferred(provider: provider, home: home)
            ?? memory.overallPreferred
        else {
            // Nothing remembered yet: Ghostty is the plain default, not a fallback.
            return LaunchTargetResolution(surface: .ghostty, fallbackReason: nil)
        }
        if capability.reach(remembered) == .attach {
            return LaunchTargetResolution(surface: remembered, fallbackReason: nil)
        }
        // The remembered surface can no longer attach — name it and why, then use
        // the embedded terminal that always can.
        let why: String
        switch capability.reach(remembered) {
        case .unavailable:
            why = "\(remembered.appName) is unavailable"
        case .worktreeOnly:
            why = "\(remembered.appName) can no longer attach"
        case .attach:
            why = ""  // unreachable: handled above
        }
        return LaunchTargetResolution(
            surface: .ghostty,
            fallbackReason: "\(why) — using the embedded terminal."
        )
    }
}
