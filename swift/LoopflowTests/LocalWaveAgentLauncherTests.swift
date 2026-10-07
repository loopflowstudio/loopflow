// The wave launcher's pure parts: how the `lf` binary resolves, how the
// development registry environment survives launch, and the not-running copy.

#if os(macOS)
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
            "LF_FLOW_ID": "launching-step",
            "LF_RUN_ID": "launching-run",
        ])

        #expect(environment["LF_HOME"] == "/tmp/loopflow-development-home")
        #expect(environment["LF_WAVE_ID"] == nil)
        #expect(environment["LF_FLOW_ID"] == nil)
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
        ]
        let environment = GUIProcessEnvironment.enriched(kept.merging(leaked) { first, _ in first })

        for (key, value) in kept where key != "PATH" { #expect(environment[key] == value) }
        #expect(environment["PATH"]?.hasSuffix("/usr/bin:/bin") == true)
        for key in leaked.keys { #expect(environment[key] == nil, "\(key) leaked") }
    }


    @Test("Task controls use the Task commands")
    func taskControlCommandShapes() {
        let lf = "/Applications/Loopflow.app/Contents/MacOS/lf"

        #expect(LocalWaveAgentLauncher.taskRunArguments(issue: "W2-131") == ["-b", "task", "run", "W2-131"])
        #expect(LocalWaveAgentLauncher.taskInterruptCommand(lfPath: lf, issue: "W2-131") == [
            lf, "task", "interrupt", "W2-131",
        ])
    }

    @Test("PR review delegates to lf pr open rather than opening a URL itself")
    func pullRequestReviewDelegatesToCLI() {
        let lf = "/Applications/Loopflow.app/Contents/MacOS/lf"
        let command = LocalWaveAgentLauncher.pullRequestReviewCommand(lfPath: lf)

        // The one review-presentation action shells `lf pr open` — the single
        // presentation boundary — and never constructs a github.com URL to open.
        #expect(command == [lf, "pr", "open"])
        #expect(!command.contains { $0.contains("github.com") })
        #expect(!command.contains { $0.hasPrefix("http") })
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

    @Test("New session prepares Task Work without running its Flow and uses the returned checkout")
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

    @Test("a missing bundled helper never falls through to PATH")
    func missingBundledHelperFails() {
        #expect {
            try LocalWaveAgentLauncher.controlLfPath(
                bundled: nil,
                developmentConfig: nil
            )
        } throws: { error in
            guard error is LocalLfError else { return false }
            return error.localizedDescription.contains("missing its executable bundled lf helper")
                && error.localizedDescription.contains("PATH fallback is disabled")
        }
    }

    @Test("a development app uses its explicit machine lf gate")
    func developmentAppUsesMachineGate() throws {
        let directory = FileManager.default.temporaryDirectory
            .appending(path: UUID().uuidString, directoryHint: .isDirectory)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let config = directory.appending(path: "LoopflowDevControl.json")
        try Data(#"{"lf_path":"/bin/sh"}"#.utf8).write(to: config)

        let path = try LocalWaveAgentLauncher.controlLfPath(
            bundled: nil,
            developmentConfig: config
        )

        #expect(path == "/bin/sh")
    }

    #if !SWIFT_PACKAGE
    @Test("the hosted app executes its bundled lf and decodes the reply")
    func hostedBundleExecutes() async throws {
        let helper = try LocalWaveAgentLauncher.controlLfPath()
        #expect(FileManager.default.isExecutableFile(atPath: helper))
        let query = RegistryQuery { args, cwd in
            try LocalWaveAgentLauncher.queryLf(args, cwd: cwd)
        }

        #expect(try await !query.localMachineId().isEmpty)
    }
    #endif

    // MARK: - Not-running copy

}

#endif
