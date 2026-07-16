import Foundation

/// A place to present an interactive handoff. Every surface can open the
/// worktree; only some can honestly *attach* the exact durable Session by
/// running the shared attach command against it.
public enum HandoffSurface: String, Codable, Sendable, Hashable, CaseIterable {
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

/// How honestly a surface reaches a handoff's Session right now.
public enum HandoffSurfaceReach: String, Codable, Sendable, Hashable {
    /// Runs the exact shared attach command against the same Session.
    case attach
    /// Opens the worktree only — a weaker action that never claims to attach.
    case worktreeOnly
    /// The app is not installed, or the surface cannot apply to this handoff.
    case unavailable
}

/// One option the surface picker offers, with the honest reach it would deliver.
/// A `worktreeOnly` reach is presented as the weaker action it is.
public struct HandoffSurfaceOption: Codable, Sendable, Hashable, Identifiable {
    public let surface: HandoffSurface
    public let reach: HandoffSurfaceReach

    public init(surface: HandoffSurface, reach: HandoffSurfaceReach) {
        self.surface = surface
        self.reach = reach
    }

    public var id: HandoffSurface { surface }

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

/// What the current machine and handoff permit. The resolver reads only these
/// facts; it never touches the filesystem or `NSWorkspace` itself, which keeps
/// the decision pure and testable.
///
/// Only a surface that runs the *exact shared attach command* may claim
/// `.attach`: Ghostty embeds it, and Warp runs it through a command-bearing
/// launch configuration. An IDE opened at a folder does not resume the specific
/// provider Session, so an IDE is never more than `.worktreeOnly` — it opens the
/// worktree without ever claiming to attach.
///
/// The descriptor's Home matters: on a **remote** Home the worktree lives on
/// another host, so a local editor or a plain local window cannot reach it.
/// Ghostty and a command-bearing Warp still attach — the launcher transports the
/// shared argv to the descriptor's host — but local worktree-only actions become
/// unavailable rather than opening a path that is not there.
public struct HandoffSurfaceCapability: Sendable, Hashable {
    /// External apps installed on the current Home. Ghostty is embedded, so it is
    /// never listed here and is always available.
    public let installedApps: Set<HandoffSurface>
    /// The worktree is a proven workspace an IDE can open (only meaningful for a
    /// local Home; a remote worktree is never locally proven).
    public let workspaceProven: Bool
    /// Warp can be handed a command-bearing launch configuration that runs the
    /// exact shared attach command.
    public let warpCommandBearing: Bool
    /// The handoff's Home is on another host (`descriptor.host`), so the worktree
    /// is not on this machine.
    public let isRemoteHome: Bool

    public init(
        installedApps: Set<HandoffSurface>,
        workspaceProven: Bool,
        warpCommandBearing: Bool,
        isRemoteHome: Bool
    ) {
        self.installedApps = installedApps
        self.workspaceProven = workspaceProven
        self.warpCommandBearing = warpCommandBearing
        self.isRemoteHome = isRemoteHome
    }

    /// How honestly `surface` can present this handoff right now.
    public func reach(_ surface: HandoffSurface) -> HandoffSurfaceReach {
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
            // A local editor cannot open a remote worktree, and no IDE launch
            // resumes the specific provider Session — so an IDE is worktree-only
            // on a local Home and unavailable on a remote one.
            guard !isRemoteHome, installedApps.contains(surface), workspaceProven else {
                return .unavailable
            }
            return .worktreeOnly
        }
    }

    /// The honest picker: every applicable surface, each labeled by its reach,
    /// in a stable order with the required Ghostty fallback first.
    public var offeredOptions: [HandoffSurfaceOption] {
        HandoffSurface.allCases.compactMap { surface in
            let reach = reach(surface)
            guard reach != .unavailable else { return nil }
            return HandoffSurfaceOption(surface: surface, reach: reach)
        }
    }
}

/// The last surface that successfully presented a handoff, remembered per
/// `(provider, Home)` and once overall. Recorded only after a launch succeeds,
/// so a failed attempt never rewrites the preference.
public struct HandoffSurfaceMemory: Codable, Sendable, Hashable {
    private var byProviderHome: [String: HandoffSurface]
    private var overall: HandoffSurface?

    public init(byProviderHome: [String: HandoffSurface] = [:], overall: HandoffSurface? = nil) {
        self.byProviderHome = byProviderHome
        self.overall = overall
    }

    // A Home address never contains a unit-separator, so it is a safe join byte.
    private static func key(provider: String, home: String) -> String {
        "\(provider)\u{1f}\(home)"
    }

    /// The surface remembered for this provider on this Home, if any.
    public func preferred(provider: String, home: String) -> HandoffSurface? {
        byProviderHome[Self.key(provider: provider, home: home)]
    }

    /// The surface remembered across every provider and Home, if any.
    public var overallPreferred: HandoffSurface? { overall }

    /// Record a surface as the new preference after its launch succeeded.
    public mutating func record(_ surface: HandoffSurface, provider: String, home: String) {
        byProviderHome[Self.key(provider: provider, home: home)] = surface
        overall = surface
    }

    /// Record a launch only when it earned the right to become Open's default.
    /// Folder-only IDE opens, automatic resolution, and failed launches leave the
    /// last attach-capable preference untouched.
    @discardableResult
    public mutating func recordLaunch(
        _ surface: HandoffSurface,
        provider: String,
        home: String,
        reach: HandoffSurfaceReach,
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
public struct HandoffSurfaceResolution: Sendable, Hashable {
    public let surface: HandoffSurface
    public let fallbackReason: String?

    public init(surface: HandoffSurface, fallbackReason: String?) {
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
public enum HandoffSurfaceResolver {
    public static func resolve(
        provider: String,
        home: String,
        memory: HandoffSurfaceMemory,
        capability: HandoffSurfaceCapability
    ) -> HandoffSurfaceResolution {
        guard let remembered = memory.preferred(provider: provider, home: home)
            ?? memory.overallPreferred
        else {
            // Nothing remembered yet: Ghostty is the plain default, not a fallback.
            return HandoffSurfaceResolution(surface: .ghostty, fallbackReason: nil)
        }
        if capability.reach(remembered) == .attach {
            return HandoffSurfaceResolution(surface: remembered, fallbackReason: nil)
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
        return HandoffSurfaceResolution(
            surface: .ghostty,
            fallbackReason: "\(why) — using the embedded terminal."
        )
    }
}
