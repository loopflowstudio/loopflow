// Launch Tasks and local sessions through the CLI.

#if os(macOS)
import Darwin
import Foundation
import Loopflow

struct LocalLfError: LocalizedError {
    let errorDescription: String?
}

enum LocalWaveAgentLauncher {
    /// Run a filed Task on its graph. `lf task run` owns graph selection,
    /// worktree placement and where to start; the app does not reproduce them.
    /// It returns once the Task's Graph is running, which keeps on without the app.
    static func runTask(repoPath: String, issue: String) async throws {
        _ = try await queryLf(["task", "run", issue], cwd: WaveOrigin.resolve(repoPath))
    }

    /// Ensure Task Work and its checkout without running a Graph; returns
    /// the authoritative worktree.
    static func checkoutTask(repoPath: String, issue: String) async throws -> WorkspaceIdentity {
        let stdout = try await runCheckedOutput(taskCheckoutCommand(lfPath: try controlLfPath(), issue: issue), cwd: repoPath)
        return try taskCheckoutWorkspace(stdout)
    }

    static func taskCheckoutCommand(lfPath: String, issue: String) -> [String] {
        [lfPath, "task", "checkout", issue, "--json"]
    }

    static func taskCheckoutWorkspace(_ stdout: String) throws -> WorkspaceIdentity {
        do {
            return try JSONDecoder().decode(WorkspaceIdentity.self, from: Data(stdout.utf8))
        } catch {
            throw LocalLfError(
                errorDescription: "lf task checkout returned an invalid receipt: \(error.localizedDescription)"
            )
        }
    }

    /// Use the app's bundled CLI, or the pinned helper inside an offline clone.
    static func controlLfPath(
        bundled: URL? = Bundle.main.url(forAuxiliaryExecutable: "lf"),
        environment: [String: String] = ProcessInfo.processInfo.environment
    ) throws -> String {
        if let root = environment["LF_CLONE_ROOT"] {
            let helper = URL(fileURLWithPath: root).appendingPathComponent("bin/lf").path
            guard environment["LF_HOME"] == root, environment["LF_BIN"] == helper,
                  FileManager.default.isExecutableFile(atPath: helper) else {
                throw LocalLfError(errorDescription: "Clone helper or Machine selection is unavailable; relaunch with lf clone run.")
            }
            return helper
        }
        guard let bundled, FileManager.default.isExecutableFile(atPath: bundled.path) else {
            throw LocalLfError(
                errorDescription: "Loopflow.app is missing its executable bundled lf helper at Contents/MacOS/lf. "
                    + "Rebuild or reinstall Loopflow; PATH fallback is disabled."
            )
        }
        return bundled.path
    }

    /// Run an `lf` query verb (`list`, `status`, `monitor`, …) and return its
    /// stdout. Backs `RegistryQuery` on macOS: the wave dashboard reads durable
    /// facts by shelling the daemonless Machine `lf` over the local store, not
    /// by streaming a center. Throws on a spawn failure or a non-zero exit.
    static func queryLf(_ subargs: [String], cwd: String?, input: String? = nil) async throws -> String {
        let lfPath = try controlLfPath()
        let result = try await run([lfPath] + subargs, cwd: cwd, input: input)
        // `lf doctor --json` exits 1 when a check fails, but its stdout is the
        // report the telemetry dashboard must show. A red monitor cannot hide
        // itself behind the query runner's generic non-zero handling.
        if subargs == ["doctor", "--json"], !result.stdout.isEmpty {
            return result.stdout
        }
        guard result.status == 0 else {
            let detail = result.stderr.trimmingCharacters(in: .whitespacesAndNewlines)
            let message = detail.isEmpty
                ? "lf \(subargs.joined(separator: " ")) failed (\(result.status))"
                : detail
            throw LocalLfError(errorDescription: message)
        }
        return result.stdout
    }

    // MARK: - Process plumbing

    static func queryProcess(_ args: [String], cwd: String? = nil) -> Process {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/env")
        process.arguments = args
        process.environment = GUIProcessEnvironment.enriched(ProcessInfo.processInfo.environment)
        if let cwd {
            process.currentDirectoryURL = URL(fileURLWithPath: cwd, isDirectory: true)
        }
        return process
    }

    static func queryGitHub(_ args: [String]) async throws -> String {
        guard ProcessInfo.processInfo.environment["LF_CLONE_ROOT"] == nil else {
            throw LocalLfError(errorDescription: "GitHub is unavailable in an offline data clone.")
        }
        return try await runCheckedOutput(["gh"] + args, cwd: nil)
    }

    private static func runCheckedOutput(_ args: [String], cwd: String?) async throws -> String {
        let result = try await run(args, cwd: cwd)
        guard result.status == 0 else {
            let detail = result.stderr.trimmingCharacters(in: .whitespacesAndNewlines)
            let message = detail.isEmpty
                ? "Command failed (\(result.status)): \(args.joined(separator: " "))"
                : detail
            throw LocalLfError(errorDescription: message)
        }
        return result.stdout
    }

    /// Finite CLI requests only. Cancellation ends this exact child, never its
    /// descendants or an execution owner. Uncertain mutations must be reconciled.
    static func run(
        _ args: [String],
        cwd: String? = nil,
        input: String? = nil
    ) async throws -> (status: Int32, stdout: String, stderr: String) {
        let worker = Task.detached(priority: .userInitiated) {
            try await collect(args, cwd: cwd, input: input)
        }
        return try await withTaskCancellationHandler {
            let result = try await worker.value
            try Task.checkCancellation()
            return result
        } onCancel: {
            worker.cancel()
        }
    }

    private static func collect(
        _ args: [String], cwd: String?, input: String?
    ) async throws -> (status: Int32, stdout: String, stderr: String) {
        try Task.checkCancellation()
        let deadline = ProcessInfo.processInfo.systemUptime + 30
        let process = queryProcess(args, cwd: cwd)
        let stdout = Pipe()
        let stderr = Pipe()
        let stdin = input.map { _ in Pipe() }
        process.standardOutput = stdout
        process.standardError = stderr
        process.standardInput = stdin.map { $0 as Any } ?? FileHandle.nullDevice
        let out = stdout.fileHandleForReading
        let err = stderr.fileHandleForReading
        defer {
            // Kill only the child we launched. A Session owner is independent;
            // closing a query must not signal a provider or a process group.
            if process.isRunning {
                _ = Darwin.kill(process.processIdentifier, SIGKILL)
                // Not waitUntilExit: it parks this cooperative thread's run
                // loop, which nothing wakes once the task has hopped threads.
                while process.isRunning { usleep(1000) }
            }
            for handle in [out, err, stdout.fileHandleForWriting, stderr.fileHandleForWriting,
                           stdin?.fileHandleForReading, stdin?.fileHandleForWriting].compactMap({ $0 }) {
                try? handle.close()
            }
        }
        do { try process.run() }
        catch { throw LocalLfError(errorDescription: "Could not start CLI request: \(error.localizedDescription)") }
        try stdout.fileHandleForWriting.close()
        try stderr.fileHandleForWriting.close()
        try stdin?.fileHandleForReading.close()
        for handle in [out, err, stdin?.fileHandleForWriting].compactMap({ $0 }) {
            let fd = handle.fileDescriptor
            guard fcntl(fd, F_SETFL, fcntl(fd, F_GETFL) | O_NONBLOCK) != -1 else {
                throw LocalLfError(errorDescription: "Could not configure CLI request pipes")
            }
            _ = fcntl(fd, F_SETNOSIGPIPE, 1)
        }
        var output = Data()
        var diagnostics = Data()
        let draft = Data((input ?? "").utf8)
        var sent = 0
        var writer = stdin?.fileHandleForWriting
        var inputError: String?
        while true {
            try Task.checkCancellation()
            guard ProcessInfo.processInfo.systemUptime < deadline else {
                throw LocalLfError(errorDescription: "CLI request timed out after 30 seconds. Its result is unknown; refresh before retrying a change.")
            }
            let running = process.isRunning
            // Bound each drain so noisy output cannot starve input/cancellation.
            let outputDrained = try drain(out, into: &output)
            let diagnosticsDrained = try drain(err, into: &diagnostics)
            if let handle = writer {
                if sent < draft.count {
                    let count = draft.withUnsafeBytes {
                        Darwin.write(handle.fileDescriptor,
                                     $0.baseAddress!.advanced(by: sent), min(draft.count - sent, 65536))
                    }
                    if count > 0 { sent += count }
                    else if count < 0 && errno != EAGAIN && errno != EINTR {
                        inputError = String(cString: strerror(errno))
                    }
                }
                if sent == draft.count || inputError != nil {
                    try handle.close()
                    writer = nil
                }
            }
            // An inherited pipe in a descendant cannot hold a completed query
            // open. The child's buffered output has been drained at this point.
            if !running, outputDrained, diagnosticsDrained { break }
            // Wake as soon as a pipe is ready; a fixed sleep drains one pipe
            // buffer per tick, too slow for a large response to beat the deadline.
            var ready = [pollfd(fd: out.fileDescriptor, events: Int16(POLLIN), revents: 0),
                         pollfd(fd: err.fileDescriptor, events: Int16(POLLIN), revents: 0)]
            if let handle = writer {
                ready.append(pollfd(fd: handle.fileDescriptor, events: Int16(POLLOUT), revents: 0))
            }
            _ = Darwin.poll(&ready, nfds_t(ready.count), 10)
            await Task.yield()
        }
        if process.terminationStatus == 0, sent != draft.count {
            throw LocalLfError(errorDescription: "Could not send draft to lf: \(inputError ?? "request exited before reading the draft")")
        }
        return (process.terminationStatus, String(decoding: output, as: UTF8.self),
                String(decoding: diagnostics, as: UTF8.self))
    }

    /// True once all currently buffered bytes have been read, including EOF.
    private static func drain(_ handle: FileHandle, into data: inout Data) throws -> Bool {
        var buffer = [UInt8](repeating: 0, count: 65536)
        for _ in 0..<16 {
            let count = Darwin.read(handle.fileDescriptor, &buffer, buffer.count)
            if count == 0 { return true }
            if count < 0 {
                if errno == EAGAIN { return true }
                if errno == EINTR { continue }
                throw LocalLfError(errorDescription: "Could not read CLI response")
            }
            guard data.count + count <= 64 * 1024 * 1024 else {
                throw LocalLfError(errorDescription: "CLI response exceeds 64 MiB")
            }
            data.append(contentsOf: buffer.prefix(count))
        }
        return false
    }
}
#endif
