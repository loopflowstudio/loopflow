#if os(macOS)
import AppKit
import Foundation
import Loopflow

/// Persists the remembered surface across launches, turning the pure
/// `HandoffSurfaceMemory` into a durable preference. Recorded only after a
/// launch succeeds, so a failed attempt never rewrites the memory.
@MainActor
@Observable
final class HandoffSurfacePreferences {
    static let shared = HandoffSurfacePreferences()

    private let defaults: UserDefaults
    private let key: String
    private(set) var memory: HandoffSurfaceMemory

    init(defaults: UserDefaults = .standard, key: String = "handoffSurfaceMemory") {
        self.defaults = defaults
        self.key = key
        if let data = defaults.data(forKey: key),
           let decoded = try? JSONDecoder().decode(HandoffSurfaceMemory.self, from: data) {
            memory = decoded
        } else {
            memory = HandoffSurfaceMemory()
        }
    }

    func record(_ surface: HandoffSurface, provider: String, home: String) {
        memory.record(surface, provider: provider, home: home)
        if let data = try? JSONEncoder().encode(memory) {
            defaults.set(data, forKey: key)
        }
    }
}

/// Detects which surfaces the current machine can present and opens them. The
/// pure resolver decides *which* surface; this is the side effect that reaches
/// out to `NSWorkspace` and the filesystem. It never creates or names a Session
/// — it runs the exact shared attach command the store handed back.
enum HandoffSurfaceLauncher {
    private static func bundleId(_ surface: HandoffSurface) -> String? {
        switch surface {
        case .ghostty: nil
        case .warp: "dev.warp.Warp-Stable"
        case .vscode: "com.microsoft.VSCode"
        case .cursor: "com.todesktop.230313mzl4w4u92"
        }
    }

    static func appURL(_ surface: HandoffSurface) -> URL? {
        guard let id = bundleId(surface) else { return nil }
        return NSWorkspace.shared.urlForApplication(withBundleIdentifier: id)
    }

    static func installedApps() -> Set<HandoffSurface> {
        Set([HandoffSurface.warp, .vscode, .cursor].filter { appURL($0) != nil })
    }

    /// Whether the descriptor's Home is on another host. A local Home is this
    /// machine (`…@local`, `localhost`, or a bare local address); anything with an
    /// `ssh://` scheme or a real remote host is remote, so a local worktree path
    /// cannot be assumed.
    static func isRemoteHome(_ host: String) -> Bool {
        let trimmed = host.trimmingCharacters(in: .whitespaces)
        if trimmed.hasPrefix("ssh://") { return true }
        if trimmed.hasSuffix("@local") || trimmed == "local" || trimmed == "localhost" {
            return false
        }
        // `user@host` with a non-local host is remote; a bare token with no host
        // part is treated as local.
        if let at = trimmed.firstIndex(of: "@") {
            let hostPart = trimmed[trimmed.index(after: at)...]
            return !(hostPart == "local" || hostPart == "localhost")
        }
        return false
    }

    /// Probe the machine and handoff into the pure capability the resolver reads.
    /// The worktree is only probed on a local Home; a remote worktree is never
    /// locally proven.
    static func capability(host: String, cwd: String) -> HandoffSurfaceCapability {
        let installed = installedApps()
        let remote = isRemoteHome(host)
        var isDirectory: ObjCBool = false
        let proven = !remote
            && FileManager.default.fileExists(atPath: cwd, isDirectory: &isDirectory)
            && isDirectory.boolValue
        return HandoffSurfaceCapability(
            installedApps: installed,
            workspaceProven: proven,
            // A command-bearing Warp launch needs only that Warp is installed; the
            // launch configuration is written on demand.
            warpCommandBearing: installed.contains(.warp),
            isRemoteHome: remote
        )
    }

    /// Launch an external surface for a handoff. Ghostty is embedded and never
    /// routed here. Returns whether the launch succeeded; the caller records the
    /// preference only on success and falls back visibly otherwise.
    @MainActor
    static func launch(
        _ surface: HandoffSurface,
        attach: InteractiveHandoffAttach,
        reach: HandoffSurfaceReach
    ) async -> Bool {
        switch surface {
        case .ghostty:
            // Embedded terminal is presented by the view, not launched here.
            return true
        case .warp:
            return launchWarp(attach: attach, attaching: reach == .attach)
        case .vscode, .cursor:
            return await openWorkspace(surface, cwd: attach.cwd)
        }
    }

    private static func launchWarp(attach: InteractiveHandoffAttach, attaching: Bool) -> Bool {
        if attaching {
            // Attach only if the command-bearing config actually gets written; a
            // failed write returns false so the caller falls back to the embedded
            // terminal rather than opening a bare window and calling it "attached".
            guard let launchURL = writeWarpLaunchConfig(attach: attach) else { return false }
            return NSWorkspace.shared.open(launchURL)
        }
        // Worktree-only: open a window at the worktree with no command. Weaker,
        // and labeled as such by the option that offered it.
        var components = URLComponents()
        components.scheme = "warp"
        components.host = "action"
        components.path = "/new_window"
        components.queryItems = [URLQueryItem(name: "path", value: attach.cwd)]
        guard let url = components.url else { return false }
        return NSWorkspace.shared.open(url)
    }

    /// The name of the Warp launch configuration for a handoff.
    static func warpLaunchConfigName(sessionId: String) -> String {
        "loopflow-handoff-\(sessionId)"
    }

    /// Render a command-bearing Warp launch configuration that runs the *exact
    /// shared attach command* in the worktree, with the descriptor's environment
    /// preserved. Pure and testable: the embedded command is the environment
    /// prefix plus the provider-session-bearing argv the store handed back, so a
    /// Warp launch attaches the same durable Session — with the same environment —
    /// rather than a fresh shell. Warp launch configs carry no environment field
    /// of their own, so the environment rides an `env KEY=VALUE …` prefix on the
    /// command itself, exactly as the embedded terminal would inherit it.
    static func warpLaunchConfigYAML(
        name: String,
        cwd: String,
        argv: [String],
        environment: [String: String] = [:]
    ) -> String {
        var tokens: [String] = []
        if !environment.isEmpty {
            tokens.append("env")
            // Stable order so the rendered command is deterministic.
            for key in environment.keys.sorted() {
                tokens.append("\(key)=\(environment[key] ?? "")")
            }
        }
        tokens.append(contentsOf: argv)
        let command = tokens.map(Self.shellQuote).joined(separator: " ")
        return """
        ---
        name: \(name)
        windows:
          - tabs:
              - layout:
                  cwd: \(Self.yamlQuote(cwd))
                  commands:
                    - exec: \(Self.yamlQuote(command))
        """
    }

    /// Write the launch configuration, returning its `warp://launch` URL, or nil
    /// if it could not be written so the caller can fall back visibly.
    private static func writeWarpLaunchConfig(attach: InteractiveHandoffAttach) -> URL? {
        let directory = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".warp/launch_configurations", isDirectory: true)
        let name = warpLaunchConfigName(sessionId: attach.sessionId)
        let yaml = warpLaunchConfigYAML(
            name: name,
            cwd: attach.cwd,
            argv: attach.argv,
            environment: attach.environment
        )
        do {
            try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
            let file = directory.appendingPathComponent("\(name).yaml")
            try yaml.write(to: file, atomically: true, encoding: .utf8)
        } catch {
            return nil
        }
        var components = URLComponents()
        components.scheme = "warp"
        components.host = "launch"
        components.path = "/\(name)"
        return components.url
    }

    @MainActor
    private static func openWorkspace(_ surface: HandoffSurface, cwd: String) async -> Bool {
        guard let appURL = appURL(surface) else { return false }
        let folder = URL(fileURLWithPath: cwd, isDirectory: true)
        do {
            _ = try await NSWorkspace.shared.open(
                [folder],
                withApplicationAt: appURL,
                configuration: NSWorkspace.OpenConfiguration()
            )
            return true
        } catch {
            return false
        }
    }

    private static func shellQuote(_ value: String) -> String {
        "'" + value.replacingOccurrences(of: "'", with: "'\\''") + "'"
    }

    private static func yamlQuote(_ value: String) -> String {
        let escaped = value
            .replacingOccurrences(of: "\\", with: "\\\\")
            .replacingOccurrences(of: "\"", with: "\\\"")
        return "\"\(escaped)\""
    }
}
#endif
