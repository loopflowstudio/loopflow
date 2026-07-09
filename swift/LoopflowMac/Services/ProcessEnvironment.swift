// Shared environment helpers for child processes spawned by the Loopflow GUI.
//
// GUI-launched apps inherit a minimal PATH (/usr/bin:/bin:/usr/sbin:/sbin) that
// doesn't include Homebrew, ~/.local/bin, or ~/.cargo/bin. Anything Loopflow
// shells out to — tmux, git, the bundled lfd, the vendor agent binaries —
// inherits that, so execvp can't find user-installed binaries. Child launchers
// await `resolved(_:)` so they see the same binaries the user's shell would.
//
// PATH starts with a fixed fallback so the app can paint immediately, then
// resolves from the user's interactive login shell in the background. Child
// launchers await that one cached resolution before spawning. A fixed candidate
// list can't keep up with version managers: nvm, pyenv, and rbenv all install
// into version-pinned directories that only exist on PATH because an rc file put
// them there. `zsh -lc` won't do — a non-interactive login shell reads .zprofile
// and skips .zshrc, which is where nvm lives. The shell must be interactive (-i)
// for its PATH to be the one the user sees.
//
// The candidate list survives as a fallback for when the shell can't be asked:
// it errors, it hangs, or it prints a PATH we can't parse.

import Foundation

actor GUIProcessEnvironment {
    static let shared = GUIProcessEnvironment()

    /// How long the login shell gets to print its PATH before we give up on it.
    /// Slow rc files are common; hanging the GUI on one is not acceptable.
    private static let shellTimeout: TimeInterval = 3

    /// Wraps the PATH so rc-file chatter on stdout can't be mistaken for it.
    private static let beginMarker = "__LF_PATH_BEGIN__"
    private static let endMarker = "__LF_PATH_END__"

    private let readLoginShellPath: @Sendable () -> String?
    private var resolutionTask: Task<[String], Never>?

    init(
        readLoginShellPath: @escaping @Sendable () -> String? = {
            GUIProcessEnvironment.loginShellPath()
        }
    ) {
        self.readLoginShellPath = readLoginShellPath
    }

    /// Directories most GUI-launched tools need even before shell discovery.
    /// Entries that don't exist are harmless on PATH.
    private static var fallbackCandidates: [String] {
        let home = FileManager.default.homeDirectoryForCurrentUser.path
        return [
            "\(home)/.local/bin",
            "/opt/homebrew/bin",
            "/opt/homebrew/sbin",
            "/usr/local/bin",
            "/usr/local/sbin",
            "\(home)/.cargo/bin",
        ]
    }

    static func fallbackPath(from existing: String?) -> String {
        mergePath(existing, prepending: fallbackCandidates)
    }

    static func fallback(_ env: [String: String]) -> [String: String] {
        var copy = env
        copy["PATH"] = fallbackPath(from: env["PATH"])
        return copy
    }

    /// Start shell discovery without waiting for it.
    func prepare() {
        _ = makeResolutionTask()
    }

    func resolvedPath(from existing: String?) async -> String {
        let shellComponents = await makeResolutionTask().value
        return Self.mergePath(
            existing,
            prepending: shellComponents + Self.fallbackCandidates
        )
    }

    func resolved(_ env: [String: String]) async -> [String: String] {
        var copy = env
        copy["PATH"] = await resolvedPath(from: env["PATH"])
        return copy
    }

    private func makeResolutionTask() -> Task<[String], Never> {
        if let resolutionTask {
            return resolutionTask
        }

        let readLoginShellPath = self.readLoginShellPath
        let task: Task<[String], Never> = Task.detached(priority: .userInitiated) {
            guard let path = readLoginShellPath() else { return [String]() }
            return path.split(separator: ":").map(String.init)
        }
        resolutionTask = task
        return task
    }

    private static func mergePath(_ existing: String?, prepending candidates: [String]) -> String {
        let existingComponents = existing?.split(separator: ":").map(String.init) ?? []

        var seen = Set(existingComponents)
        var prepended: [String] = []
        for dir in candidates where !seen.contains(dir) {
            prepended.append(dir)
            seen.insert(dir)
        }
        return (prepended + existingComponents).joined(separator: ":")
    }

    /// Run the user's shell as an interactive login shell and read back its
    /// PATH. Returns nil if the shell fails, times out, or prints nothing we
    /// recognize — callers fall back to the candidate list.
    private static func loginShellPath() -> String? {
        let shell = userShell()
        guard FileManager.default.isExecutableFile(atPath: shell) else { return nil }

        let process = Process()
        process.executableURL = URL(fileURLWithPath: shell)
        process.arguments = [
            "-ilc",
            "/usr/bin/printf '%s' '\(beginMarker)'; "
                + "/usr/bin/printenv PATH; "
                + "/usr/bin/printf '%s' '\(endMarker)'",
        ]

        // A pipe would have to be drained concurrently with the timeout wait;
        // reading it synchronously can block forever before the timeout fires.
        // A temporary file lets rc-file chatter flow without coupling shell
        // termination to our reader.
        let stdoutURL = FileManager.default.temporaryDirectory
            .appendingPathComponent("loopflow-shell-path-\(UUID().uuidString)")
        guard FileManager.default.createFile(atPath: stdoutURL.path, contents: nil),
              let stdout = try? FileHandle(forWritingTo: stdoutURL)
        else { return nil }
        defer {
            try? stdout.close()
            try? FileManager.default.removeItem(at: stdoutURL)
        }
        process.standardOutput = stdout
        // rc files write banners, warnings, and version-manager noise to stderr.
        // None of it is ours to surface.
        process.standardError = FileHandle.nullDevice
        process.standardInput = FileHandle.nullDevice

        // Registered before run(): a shell that exits before we get here would
        // never fire a handler installed afterwards, and we'd wait out the full
        // timeout on every launch.
        let exited = DispatchSemaphore(value: 0)
        process.terminationHandler = { _ in exited.signal() }

        do {
            try process.run()
        } catch {
            return nil
        }

        guard exited.wait(timeout: .now() + shellTimeout) == .success else {
            process.terminate()
            return nil
        }
        guard process.terminationStatus == 0 else { return nil }
        guard let data = try? Data(contentsOf: stdoutURL) else { return nil }

        return extractPath(from: String(data: data, encoding: .utf8) ?? "")
    }

    /// The user's real shell. The GUI environment's SHELL is set from the same
    /// directory-services record, but it isn't guaranteed to be there.
    private static func userShell() -> String {
        if let shell = ProcessInfo.processInfo.environment["SHELL"], !shell.isEmpty {
            return shell
        }
        if let pw = getpwuid(getuid()), let shell = pw.pointee.pw_shell {
            return String(cString: shell)
        }
        return "/bin/zsh"
    }

    /// Pull the PATH out from between the markers, ignoring anything an rc file
    /// printed around it.
    private static func extractPath(from output: String) -> String? {
        guard let begin = output.range(of: beginMarker),
              let end = output.range(of: endMarker, range: begin.upperBound..<output.endIndex)
        else { return nil }

        let path = String(output[begin.upperBound..<end.lowerBound])
            .trimmingCharacters(in: .newlines)
        return path.isEmpty ? nil : path
    }
}
