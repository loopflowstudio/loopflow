#if os(macOS)
import Foundation
import LoopflowCore

enum LocalWaveAgentLauncher {
    static func attach(repoPath: String, waveName: String) async throws -> SessionConnectionInfo {
        let handle = try await launchWaveAgent(repoPath: repoPath, waveName: waveName)
        return SessionConnectionInfo(
            kind: "tmux",
            sessionName: handle,
            host: "localhost",
            cwd: PortfolioRepoState.waveWorktreePath(repoPath: repoPath, waveName: waveName),
            status: .running
        )
    }

    static func sessionExists(repoPath: String, waveName: String) -> Bool {
        runCommandSync(
            [
                "tmux",
                "has-session",
                "-t",
                PortfolioRepoState.waveAgentSessionName(repoPath: repoPath, waveName: waveName),
            ],
            logFailure: false
        ) != nil
    }

    private static func launchWaveAgent(repoPath: String, waveName: String) async throws -> String {
        try await Task.detached {
            let lfURL = try bundledExecutable(named: "lf")
            let (output, status) = try launchCapturingHandle(
                [lfURL.path, "goal", waveName, "--tmux"],
                cwd: repoPath
            )
            guard status == 0,
                  let handle = output
                      .split(whereSeparator: \.isNewline)
                      .last
                      .map(String.init),
                  !handle.isEmpty else {
                throw WaveServiceError.commandFailed("lf goal \(waveName) --tmux did not print a tmux handle.")
            }
            return handle
        }.value
    }

    /// Poll a temp file for the printed tmux handle instead of reading a pipe to
    /// EOF. Detached tmux servers can inherit stdout, which keeps pipes open for
    /// the session lifetime even after the launcher has printed its handle.
    private static func launchCapturingHandle(
        _ args: [String],
        cwd: String,
        timeout: TimeInterval = 15
    ) throws -> (output: String, status: Int32) {
        let tmp = FileManager.default.temporaryDirectory
            .appendingPathComponent("lf-launch-\(UUID().uuidString)")
        FileManager.default.createFile(atPath: tmp.path, contents: nil)
        let sink = try FileHandle(forWritingTo: tmp)
        defer {
            try? sink.close()
            try? FileManager.default.removeItem(at: tmp)
        }

        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/env")
        process.arguments = args
        process.standardOutput = sink
        process.standardError = FileHandle.nullDevice
        process.environment = GUIProcessEnvironment.enriched(ProcessInfo.processInfo.environment)
        process.currentDirectoryURL = URL(fileURLWithPath: cwd, isDirectory: true)

        try process.run()

        func captured() -> String {
            let data = (try? Data(contentsOf: tmp)) ?? Data()
            return String(data: data, encoding: .utf8) ?? ""
        }

        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            let text = captured()
            if text.contains(where: \.isNewline) {
                return (text, 0)
            }
            if !process.isRunning {
                return (text, process.terminationStatus)
            }
            Thread.sleep(forTimeInterval: 0.1)
        }
        if process.isRunning { process.terminate() }
        return (captured(), 1)
    }

    private static func bundledExecutable(named name: String) throws -> URL {
        if let url = Bundle.main.url(forAuxiliaryExecutable: name) {
            return url
        }
        if let envPath = ProcessInfo.processInfo.environment["CONCERTO_\(name.uppercased())_BIN"],
           !envPath.isEmpty {
            return URL(fileURLWithPath: envPath)
        }
        throw WaveServiceError.commandFailed("Missing bundled executable: \(name)")
    }

    private static func runCommandSync(
        _ args: [String],
        cwd: String? = nil,
        logFailure: Bool = true
    ) -> String? {
        let process = Process()
        let stdout = Pipe()
        let stderr = Pipe()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/env")
        process.arguments = args
        process.standardOutput = stdout
        process.standardError = stderr
        process.environment = GUIProcessEnvironment.enriched(ProcessInfo.processInfo.environment)
        if let cwd {
            process.currentDirectoryURL = URL(fileURLWithPath: cwd, isDirectory: true)
        }

        do {
            try process.run()
            process.waitUntilExit()
        } catch {
            return nil
        }

        let stdoutText = String(data: stdout.fileHandleForReading.readDataToEndOfFile(), encoding: .utf8) ?? ""
        let stderrText = String(data: stderr.fileHandleForReading.readDataToEndOfFile(), encoding: .utf8) ?? ""
        guard process.terminationStatus == 0 else {
            guard logFailure else { return nil }
            LoggingService.lfd(
                "command failed: \(args.joined(separator: " ")) \(stderrText.trimmingCharacters(in: .whitespacesAndNewlines))"
            )
            return nil
        }
        return stdoutText
    }
}
#endif
