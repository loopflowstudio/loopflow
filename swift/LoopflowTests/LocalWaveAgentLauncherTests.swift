// The wave launcher's pure parts: how the `lf` binary resolves, how the
// development registry environment survives launch, and the not-running copy.

#if os(macOS)
import Darwin
import Foundation
import Testing
@testable import Loopflow
@testable import LoopflowMac

@Suite("Local wave launcher")
struct LocalWaveAgentLauncherTests {
    // MARK: - Command construction

    @Test("development launcher preserves the selected Machine registry")
    func launchEnvironmentPreservesRegistry() {
        let environment = GUIProcessEnvironment.enriched([
            "PATH": "/usr/bin:/bin",
            "LF_HOME": "/tmp/loopflow-development-home",
            "LF_WAVE_ID": "launching-wave",
            "LF_GRAPH_ID": "launching-step",
            "LF_RUN_ID": "launching-run",
        ])

        #expect(environment["LF_HOME"] == "/tmp/loopflow-development-home")
        #expect(environment["LF_WAVE_ID"] == nil)
        #expect(environment["LF_GRAPH_ID"] == nil)
        #expect(environment["LF_RUN_ID"] == nil)
    }

    @Test("the launcher's terminal and agent output policy are dropped; account authority stays")
    func launchEnvironmentDropsLauncherTerminal() throws {
        let kept = [
            "PATH": "/usr/bin:/bin",
            "HOME": "/Users/someone",
            "LF_HOME": "/tmp/loopflow-development-home",
            "LF_ACCOUNT": "work",
            "CODEX_HOME": "/tmp/codex-home",
            "CODEX_ACCESS_TOKEN": "codex-token",
            "CODEX_API_KEY": "codex-key",
            "CLAUDE_CODE_OAUTH_TOKEN": "claude-token",
        ]
        let leaked = [
            "NO_COLOR": "1", "FORCE_COLOR": "0", "CLICOLOR": "0", "CLICOLOR_FORCE": "0",
            "COLORTERM": "24bit", "TERM": "dumb", "TERM_PROGRAM": "WarpTerminal",
            "TERM_PROGRAM_VERSION": "v0", "TMUX": "/tmp/tmux", "TMUX_PANE": "%1",
            "CI": "1", "PAGER": "cat", "GIT_PAGER": "cat", "GH_PAGER": "cat",
            "AI_AGENT": "codex", "CLAUDECODE": "1", "WARP_IS_LOCAL_SHELL_SESSION": "1",
            "CLAUDE_CODE_ENTRYPOINT": "cli", "CODEX_CI": "1", "CODEX_THREAD_ID": "t",
            "CMUX_WORKSPACE_ID": "launching-workspace", "CMUX_SURFACE_ID": "launching-surface",
            "CMUX_SOCKET_PATH": "/tmp/cmux.sock", "HERDR_ENV": "1", "HERDR_SOCKET_PATH": "/tmp/herdr.sock",
            "HERDR_PANE_ID": "w1:p1",
        ]
        let environment = GUIProcessEnvironment.enriched(kept.merging(leaked) { first, _ in first })

        for (key, value) in kept where key != "PATH" { #expect(environment[key] == value) }
        #expect(environment["PATH"]?.hasSuffix("/usr/bin:/bin") == true)
        for key in leaked.keys { #expect(environment[key] == nil, "\(key) leaked") }
        // `lf open` and `lf desktop` from a Desktop terminal address this app.
        #expect(environment["LF_DESKTOP"] == Bundle.main.bundleIdentifier ?? "com.loopflow.mac")
    }


    @Test("Prepared checkout receipts retain the owning Machine and require its evidence")
    func checkoutIdentityFixture() throws {
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        let json = try String(contentsOf: root.appendingPathComponent("tests/fixtures/dto/task_checkout.json"), encoding: .utf8)
        #expect(try LocalWaveAgentLauncher.taskCheckoutWorkspace(json) == fixtureWorkspace("/src/loopflow.workspace"))
        #expect(throws: LocalLfError.self) {
            try LocalWaveAgentLauncher.taskCheckoutWorkspace("{\"worktree\":\"/repo\"}")
        }
    }

    @Test("New session prepares Task Work without running its Graph and uses the returned checkout")
    func taskCheckoutUsesReceiptWorktree() throws {
        #expect(LocalWaveAgentLauncher.taskCheckoutCommand(lfPath: "/bin/lf", issue: "LOO-291")
            == ["/bin/lf", "task", "checkout", "LOO-291", "--json"])
        let worktree = try LocalWaveAgentLauncher.taskCheckoutWorkspace("""
        {"machine_id": "home_00000000000000000000000000000001", "id": "task_1", "issue": "LOO-291", "worktree": "/src/loopflow.main-view-task", "agent": "claude"}
        """)
        #expect(worktree == fixtureWorkspace("/src/loopflow.main-view-task"))
        #expect(throws: LocalLfError.self) { try LocalWaveAgentLauncher.taskCheckoutWorkspace("not json") }
    }

    // MARK: - Bundled binary boundary

    @Test("a private home selects its explicit helper before the bundled one")
    func privateHomeSelectsExplicitHelper() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        let bin = directory.appendingPathComponent("bin")
        try FileManager.default.createDirectory(at: bin, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let helper = bin.appendingPathComponent("lf")
        try Data("#!/bin/sh\nexit 0\n".utf8).write(to: helper)
        try FileManager.default.setAttributes([.posixPermissions: 0o755], ofItemAtPath: helper.path)
        let environment = ["LF_HOME": directory.path, "LF_BIN": helper.path]
        #expect(try LocalWaveAgentLauncher.controlLfPath(bundled: URL(fileURLWithPath: "/bin/sh"), environment: environment) == helper.path)
        try FileManager.default.removeItem(at: helper)
        #expect(throws: LocalLfError.self) {
            try LocalWaveAgentLauncher.controlLfPath(bundled: URL(fileURLWithPath: "/bin/sh"), environment: environment)
        }
    }

    @Test("a missing bundled helper never falls through to PATH")
    func missingBundledHelperFails() {
        #expect {
            try LocalWaveAgentLauncher.controlLfPath(bundled: nil, environment: [:])
        } throws: { error in
            guard error is LocalLfError else { return false }
            return error.localizedDescription.contains("missing its executable bundled lf helper")
                && error.localizedDescription.contains("PATH fallback is disabled")
        }
    }

    @Test("the app selects its bundled helper")
    func appUsesBundledHelper() throws {
        #expect(try LocalWaveAgentLauncher.controlLfPath(bundled: URL(fileURLWithPath: "/bin/sh"), environment: [:]) == "/bin/sh")
    }

    #if !SWIFT_PACKAGE
    @Test("the hosted app executes its bundled lf and decodes the reply")
    func hostedBundleExecutes() async throws {
        let helper = try LocalWaveAgentLauncher.controlLfPath()
        #expect(FileManager.default.isExecutableFile(atPath: helper))
        let query = RegistryQuery { args, cwd in
            try await LocalWaveAgentLauncher.queryLf(args, cwd: cwd)
        }

        #expect(try await !query.localMachineId().isEmpty)
    }
    #endif

    @Test("finite queries drain both full pipes while sending a large draft")
    func queryDrainsWhileWriting() async throws {
        let draft = String(repeating: "draft\n", count: 32768)
        let result = try await LocalWaveAgentLauncher.run([
            "/bin/sh", "-c",
            "/usr/bin/head -c 131072 /dev/zero; /usr/bin/head -c 131072 /dev/zero >&2; /bin/cat"
        ], input: draft)
        #expect(result.status == 0)
        #expect(result.stdout == String(repeating: "\0", count: 131072) + draft)
        #expect(result.stderr == String(repeating: "\0", count: 131072))
    }

    @Test("canceling a request reaps only its CLI child")
    func queryCancellationLeavesDescendant() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let request = Task {
            try await LocalWaveAgentLauncher.run([
                "/bin/sh", "-c",
                "trap '' TERM; /bin/sleep 60 & echo $! > descendant; echo $$ > child; wait"
            ], cwd: directory.path, input: String(repeating: "unread draft", count: 65536))
        }
        defer { request.cancel() }
        let child = try await readPID(directory.appendingPathComponent("child"))
        let descendant = try await readPID(directory.appendingPathComponent("descendant"))
        defer { _ = Darwin.kill(descendant, SIGKILL) }
        let started = ContinuousClock.now
        request.cancel()
        do {
            _ = try await request.value
            Issue.record("Canceled request returned success")
        } catch is CancellationError {} // cancellation remains distinguishable from failure
        #expect(started.duration(to: .now) < .seconds(5))
        #expect(Darwin.kill(child, 0) == -1)
        #expect(Darwin.kill(descendant, 0) == 0)
    }

    @Test("oversized CLI responses fail instead of retaining unbounded output",
          arguments: [false, true])
    func queryOutputLimit(stderr: Bool) async throws {
        do {
            _ = try await LocalWaveAgentLauncher.run([
                "/bin/sh", "-c",
                "exec /usr/bin/head -c 67108865 /dev/zero" + (stderr ? " >&2" : "")
            ])
            Issue.record("Oversized response returned success")
        } catch {
            #expect(error.localizedDescription.contains("CLI response exceeds 64 MiB"))
        }
    }

    @Test("query completion does not await inherited pipes in surviving descendants")
    func queryCompletesWithInheritedPipes() async throws {
        let result = try await LocalWaveAgentLauncher.run([
            "/bin/sh", "-c", "/bin/sleep 60 & echo $!; printf diagnostic >&2; exit 7"
        ])
        let descendant = try #require(Int32(result.stdout.trimmingCharacters(in: .whitespacesAndNewlines)))
        defer { _ = Darwin.kill(descendant, SIGKILL) }
        #expect(result.status == 7)
        #expect(result.stderr == "diagnostic")
        #expect(Darwin.kill(descendant, 0) == 0)
    }

    @Test("a stalled CLI request times out and reaps its child")
    func queryDeadline() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let request = Task {
            try await LocalWaveAgentLauncher.run([
                "/bin/sh", "-c", "echo $$ > child; exec /bin/sleep 60"
            ], cwd: directory.path)
        }
        let child = try await readPID(directory.appendingPathComponent("child"))
        defer { request.cancel() }
        do {
            _ = try await request.value
            Issue.record("Stalled request returned success")
        } catch {
            #expect(error.localizedDescription.contains("timed out after 30 seconds"))
            #expect(error.localizedDescription.contains("result is unknown"))
        }
        #expect(Darwin.kill(child, 0) == -1)
    }

    private func readPID(_ url: URL) async throws -> Int32 {
        let deadline = ContinuousClock.now + .seconds(5)
        while ContinuousClock.now < deadline {
            if let text = try? String(contentsOf: url, encoding: .utf8),
               let pid = Int32(text.trimmingCharacters(in: .whitespacesAndNewlines)) { return pid }
            try await Task.sleep(for: .milliseconds(10))
        }
        throw LocalLfError(errorDescription: "Query fixture did not start")
    }
}

#endif
