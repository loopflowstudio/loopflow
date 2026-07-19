#if os(macOS)
import AppKit
import Foundation
import Loopflow

/// Persists the remembered surface across launches, turning the pure
/// `LaunchTargetMemory` into a durable preference. Recorded only after a
/// launch succeeds, so a failed attempt never rewrites the memory.
@MainActor
@Observable
final class LaunchTargetPreferences {
    static let shared = LaunchTargetPreferences()

    private let defaults: UserDefaults
    private let key: String
    private(set) var memory: LaunchTargetMemory

    init(defaults: UserDefaults = .standard, key: String = "launchSurfaceMemory") {
        self.defaults = defaults
        self.key = key
        if let data = defaults.data(forKey: key),
           let decoded = try? JSONDecoder().decode(LaunchTargetMemory.self, from: data) {
            memory = decoded
        } else {
            memory = LaunchTargetMemory()
        }
    }

    func recordLaunch(
        _ surface: LaunchTarget,
        provider: String,
        home: String,
        reach: LaunchTargetReach,
        userInitiated: Bool,
        launchSucceeded: Bool
    ) {
        guard memory.recordLaunch(
            surface,
            provider: provider,
            home: home,
            reach: reach,
            userInitiated: userInitiated,
            launchSucceeded: launchSucceeded
        ) else { return }
        if let data = try? JSONEncoder().encode(memory) {
            defaults.set(data, forKey: key)
        }
    }
}

/// The outcome of launching a surface for a launch. A launch can fully attach
/// (the exact shared command ran), partially succeed (the app opened but the
/// command could not run — worktree-only), or fail (nothing opened, fall back).
enum LaunchLaunchResult: Equatable, Sendable {
    case attached
    case worktreeOnly
    case failed
}

/// Detects which surfaces the current machine can present and opens them. The
/// pure resolver decides *which* surface; this is the side effect that reaches
/// out to `NSWorkspace` and the filesystem. It never creates or names a Session
/// — it runs the exact shared attach command the store handed back.
enum LaunchTargetLauncher {
    struct Command: Equatable {
        let cwd: String
        let argv: [String]
        let environment: [String: String]
    }

    private static func bundleId(_ surface: LaunchTarget) -> String? {
        switch surface {
        case .ghostty: nil
        case .warp: "dev.warp.Warp-Stable"
        case .vscode: "com.microsoft.VSCode"
        case .cursor: "com.todesktop.230313mzl4w4u92"
        }
    }

    static func appURL(_ surface: LaunchTarget) -> URL? {
        guard let id = bundleId(surface) else { return nil }
        return NSWorkspace.shared.urlForApplication(withBundleIdentifier: id)
    }

    static func installedApps() -> Set<LaunchTarget> {
        Set([LaunchTarget.warp, .vscode, .cursor].filter { appURL($0) != nil })
    }

    /// Whether the descriptor's execution Home is another machine. Rust emits
    /// `<owner>@local` locally and an SSH address remotely.
    static func isRemoteHome(_ host: String) -> Bool {
        let trimmed = host.trimmingCharacters(in: .whitespaces)
        return !trimmed.isEmpty
            && trimmed != "localhost"
            && trimmed != "local"
            && !trimmed.hasSuffix("@local")
            && trimmed != "127.0.0.1"
            && trimmed != "::1"
    }

    /// Adapt the store's host-local descriptor for this presentation Home. Local
    /// descriptors remain byte-for-byte unchanged. Remote descriptors run the
    /// exact cwd/environment/argv on their declared host instead of probing or
    /// executing the remote path on this Mac.
    static func command(for attach: LaunchSurfaceRecord, home: String? = nil) -> Command {
        guard isRemoteHome(attach.host) else {
            return Command(cwd: attach.cwd, argv: attach.argv, environment: attach.environment)
        }

        var remoteTokens = ["env"]
        for key in attach.environment.keys.sorted() {
            remoteTokens.append("\(key)=\(attach.environment[key] ?? "")")
        }
        remoteTokens.append(contentsOf: attach.argv)
        let remoteCommand = "cd \(shellQuote(attach.cwd)) && exec "
            + remoteTokens.map(shellQuote).joined(separator: " ")
        return Command(
            cwd: "/",
            argv: sshArgv(host: attach.host, home: home, remoteCommand: remoteCommand),
            environment: [:]
        )
    }

    /// Keep the Feedback controller local. `lf launch present` owns the one Home
    /// hop needed to reach the recorded provider terminal.
    static func feedbackCommand(for attach: LaunchSurfaceRecord) -> Command {
        let lfPath = Bundle.main.url(forAuxiliaryExecutable: "lf")?.path ?? "lf"
        return Command(
            cwd: "/",
            argv: [
                lfPath, "work", "feedback", attach.work.kind.rawValue, attach.work.id,
                "--continue-on-exit",
            ],
            environment: [:]
        )
    }

    /// Probe the machine and launch into the pure capability the resolver reads.
    /// The worktree is only probed on a local Home; a remote worktree is never
    /// locally proven. The provider and session id determine whether an IDE can
    /// attach (Claude with a known session id) or is worktree-only.
    static func capability(
        host: String,
        cwd: String,
        provider: String,
        providerSessionId: String?
    ) -> LaunchTargetCapability {
        let installed = installedApps()
        let remote = isRemoteHome(host)
        var isDirectory: ObjCBool = false
        let proven = !remote
            && FileManager.default.fileExists(atPath: cwd, isDirectory: &isDirectory)
            && isDirectory.boolValue
        return LaunchTargetCapability(
            installedApps: installed,
            workspaceProven: proven,
            // A command-bearing Warp launch needs only that Warp is installed; the
            // launch configuration is written on demand.
            warpCommandBearing: installed.contains(.warp),
            isRemoteHome: remote,
            providerIsClaude: provider == "claude",
            providerSessionKnown: providerSessionId != nil
        )
    }

    /// Launch an external surface for a launch. Ghostty is embedded and never
    /// routed here. Returns whether the launch attached, opened worktree-only,
    /// or failed; the caller records the preference only on `.attached` and
    /// falls back visibly on `.failed`.
    @MainActor
    static func launch(
        _ surface: LaunchTarget,
        attach: LaunchSurfaceRecord,
        home: String,
        reach: LaunchTargetReach
    ) async -> LaunchLaunchResult {
        switch surface {
        case .ghostty:
            // Embedded terminal is presented by the view, not launched here.
            return .attached
        case .warp:
            return launchWarp(attach: attach, home: home, attaching: reach == .attach)
        case .vscode, .cursor:
            if reach == .attach {
                return await launchIDEAttach(surface, attach: attach, home: home)
            }
            return await openWorkspace(surface, cwd: attach.cwd) ? .worktreeOnly : .failed
        }
    }

    private static func launchWarp(
        attach: LaunchSurfaceRecord,
        home: String,
        attaching: Bool
    ) -> LaunchLaunchResult {
        if attaching {
            // Attach only if the command-bearing config actually gets written; a
            // failed write returns .failed so the caller falls back to the embedded
            // terminal rather than opening a bare window and calling it "attached".
            guard let launchURL = writeWarpLaunchConfig(attach: attach, home: home) else { return .failed }
            return NSWorkspace.shared.open(launchURL) ? .attached : .failed
        }
        // Worktree-only: open a window at the worktree with no command. Weaker,
        // and labeled as such by the option that offered it.
        var components = URLComponents()
        components.scheme = "warp"
        components.host = "action"
        components.path = "/new_window"
        components.queryItems = [URLQueryItem(name: "path", value: attach.cwd)]
        guard let url = components.url else { return .failed }
        return NSWorkspace.shared.open(url) ? .worktreeOnly : .failed
    }

    /// The name of the Warp launch configuration for a launch.
    static func warpLaunchConfigName(sessionId: String) -> String {
        "loopflow-launch-\(sessionId)"
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
    private static func writeWarpLaunchConfig(attach: LaunchSurfaceRecord, home: String) -> URL? {
        let directory = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".warp/launch_configurations", isDirectory: true)
        let name = warpLaunchConfigName(sessionId: attach.sessionId)
        let command = command(for: attach, home: home)
        let yaml = warpLaunchConfigYAML(
            name: name,
            cwd: command.cwd,
            argv: command.argv,
            environment: command.environment
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
    private static func openWorkspace(_ surface: LaunchTarget, cwd: String) async -> Bool {
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

    /// Open the IDE at the worktree, then run the exact shared attach command in
    /// its integrated terminal via AppleScript. The IDE is opened first so the
    /// `--ide` flag (if present in the argv) can auto-connect. If the AppleScript
    /// cannot run the command (e.g. Accessibility permission denied), the IDE is
    /// already open at the worktree — the launch is worktree-only, not failed,
    /// so the user sees the honest weaker action instead of a fallback.
    @MainActor
    private static func launchIDEAttach(
        _ surface: LaunchTarget,
        attach: LaunchSurfaceRecord,
        home: String
    ) async -> LaunchLaunchResult {
        guard await openWorkspace(surface, cwd: attach.cwd) else { return .failed }
        let command = command(for: attach, home: home)
        let script = ideAttachAppleScript(
            bundleName: surface.appName,
            shellCommand: ideShellCommand(from: command)
        )
        let appleScript = NSAppleScript(source: script)
        var errorInfo: NSDictionary?
        appleScript?.executeAndReturnError(&errorInfo)
        if errorInfo != nil { return .worktreeOnly }
        return .attached
    }

    /// Build the shell command that runs in the IDE's integrated terminal: set
    /// the environment (if any), cd to the worktree, and exec the exact argv.
    static func ideShellCommand(from command: Command) -> String {
        var tokens: [String] = []
        if !command.environment.isEmpty {
            tokens.append("env")
            for key in command.environment.keys.sorted() {
                tokens.append("\(key)=\(command.environment[key] ?? "")")
            }
        }
        tokens.append(contentsOf: command.argv)
        let execCommand = tokens.map(shellQuote).joined(separator: " ")
        if command.cwd != "/" {
            return "cd \(shellQuote(command.cwd)) && exec \(execCommand)"
        }
        return execCommand
    }

    /// AppleScript that activates the IDE, opens a new integrated terminal via
    /// the command palette, and runs the shell command in it. The command
    /// palette path is reliable across keybinding customizations — "Terminal:
    /// Create New Integrated Terminal" always creates a fresh terminal.
    static func ideAttachAppleScript(bundleName: String, shellCommand: String) -> String {
        let escaped = shellCommand
            .replacingOccurrences(of: "\\", with: "\\\\")
            .replacingOccurrences(of: "\"", with: "\\\"")
        return """
        tell application "\(bundleName)" to activate
        delay 0.5
        tell application "System Events"
            keystroke "p" using {command down, shift down}
            delay 0.3
            keystroke "Terminal: Create New Integrated Terminal"
            delay 0.2
            key code 36
            delay 0.5
            keystroke "\(escaped)"
            key code 36
        end tell
        """
    }

    private static func shellQuote(_ value: String) -> String {
        "'" + value.replacingOccurrences(of: "'", with: "'\\''") + "'"
    }

    private static func sshArgv(host: String, home: String?, remoteCommand: String) -> [String] {
        let (hostname, port) = splitHostAndPort(host)
        let destination: String
        if let home,
           home.hasPrefix("ssh://"),
           let at = home.dropFirst("ssh://".count).firstIndex(of: "@") {
            let owner = home.dropFirst("ssh://".count)[..<at]
            destination = "\(owner)@\(hostname)"
        } else {
            destination = hostname
        }
        var argv = ["ssh"]
        if let port {
            argv.append(contentsOf: ["-p", port])
        }
        argv.append(contentsOf: [destination, remoteCommand])
        return argv
    }

    private static func splitHostAndPort(_ host: String) -> (String, String?) {
        if host.hasPrefix("["), let bracket = host.firstIndex(of: "]") {
            let destination = String(host[host.index(after: host.startIndex) ..< bracket])
            let suffix = host[host.index(after: bracket)...]
            if suffix.hasPrefix(":"), suffix.dropFirst().allSatisfy(\.isNumber) {
                return (destination, String(suffix.dropFirst()))
            }
            return (destination, nil)
        }
        guard host.filter({ $0 == ":" }).count == 1,
              let colon = host.lastIndex(of: ":"),
              host[host.index(after: colon)...].allSatisfy(\.isNumber)
        else { return (host, nil) }
        return (
            String(host[..<colon]),
            String(host[host.index(after: colon)...])
        )
    }

    private static func yamlQuote(_ value: String) -> String {
        let escaped = value
            .replacingOccurrences(of: "\\", with: "\\\\")
            .replacingOccurrences(of: "\"", with: "\\\"")
        return "\"\(escaped)\""
    }
}
#endif
