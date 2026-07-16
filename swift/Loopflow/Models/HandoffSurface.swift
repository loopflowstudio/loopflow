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
public struct HandoffSurfaceCapability: Sendable, Hashable {
    /// External apps installed on the current Home. Ghostty is embedded, so it is
    /// never listed here and is always available.
    public let installedApps: Set<HandoffSurface>
    /// The handoff's provider is Claude — IDE attach names Claude.
    public let providerIsClaude: Bool
    /// A provider session id is known, so an IDE can claim to attach.
    public let providerSessionKnown: Bool
    /// The worktree is a proven workspace an IDE can open.
    public let workspaceProven: Bool
    /// Warp can be handed a command-bearing launch configuration.
    public let warpCommandBearing: Bool

    public init(
        installedApps: Set<HandoffSurface>,
        providerIsClaude: Bool,
        providerSessionKnown: Bool,
        workspaceProven: Bool,
        warpCommandBearing: Bool
    ) {
        self.installedApps = installedApps
        self.providerIsClaude = providerIsClaude
        self.providerSessionKnown = providerSessionKnown
        self.workspaceProven = workspaceProven
        self.warpCommandBearing = warpCommandBearing
    }

    /// How honestly `surface` can present this handoff right now.
    public func reach(_ surface: HandoffSurface) -> HandoffSurfaceReach {
        switch surface {
        case .ghostty:
            // Embedded and required: always the honest attach fallback.
            return .attach
        case .warp:
            guard installedApps.contains(.warp) else { return .unavailable }
            return warpCommandBearing ? .attach : .worktreeOnly
        case .vscode, .cursor:
            guard installedApps.contains(surface) else { return .unavailable }
            // Claude-in-IDE attach needs a session id and a proven workspace;
            // anything short of that opens the worktree without claiming to attach.
            if providerIsClaude && providerSessionKnown && workspaceProven {
                return .attach
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
}

/// Decides which surface Open uses by default. The decision is pure: the
/// remembered surface for this provider on this Home, then the remembered
/// overall surface, then the embedded Ghostty fallback. A remembered surface is
/// honored only while it can still `.attach`, so an uninstalled app or a lost
/// capability falls back visibly to the next candidate rather than opening a
/// surface that would lie about reaching the Session.
public enum HandoffSurfaceResolver {
    public static func resolve(
        provider: String,
        home: String,
        memory: HandoffSurfaceMemory,
        capability: HandoffSurfaceCapability
    ) -> HandoffSurface {
        let remembered = [
            memory.preferred(provider: provider, home: home),
            memory.overallPreferred,
        ]
        for case let surface? in remembered where capability.reach(surface) == .attach {
            return surface
        }
        return .ghostty
    }
}
