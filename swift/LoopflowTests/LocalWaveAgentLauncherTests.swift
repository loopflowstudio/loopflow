// The wave launcher's pure parts: how the `lf` binary resolves, how the
// development registry environment survives launch, and the not-running copy.

#if os(macOS)
import Foundation
import Testing
@testable import Loopflow
@testable import LoopflowMac

@Suite("Local wave launcher")
struct LocalWaveAgentLauncherTests {
    @Test("clone terminals retain the isolated helper and provider selection")
    func cloneEnvironmentStaysIsolated() {
        let selected = [
            "LF_CLONE_ROOT": "/tmp/clone", "LF_HOME": "/tmp/clone",
            "LF_BIN": "/tmp/clone/bin/lf", "HOME": "/tmp/clone/home",
            "CODEX_HOME": "/tmp/clone/home/.codex",
            "CLAUDE_CONFIG_DIR": "/tmp/clone/home/.claude",
            "PATH": "/tmp/clone/bin:/usr/bin:/bin",
        ]
        let environment = GUIProcessEnvironment.enriched(selected)
        for (key, value) in selected { #expect(environment[key] == value) }
    }

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
            "CMUX_WORKSPACE_ID": "launching-workspace", "CMUX_SURFACE_ID": "launching-surface",
            "CMUX_SOCKET_PATH": "/tmp/cmux.sock", "HERDR_ENV": "1", "HERDR_SOCKET_PATH": "/tmp/herdr.sock",
            "HERDR_PANE_ID": "w1:p1",
        ]
        let environment = GUIProcessEnvironment.enriched(kept.merging(leaked) { first, _ in first })

        for (key, value) in kept where key != "PATH" { #expect(environment[key] == value) }
        #expect(environment["PATH"]?.hasSuffix("/usr/bin:/bin") == true)
        for key in leaked.keys { #expect(environment[key] == nil, "\(key) leaked") }
        // `lf open` from a Desktop terminal opens in Desktop, not the launcher's host.
        #expect(environment["LF_DESKTOP"] == "1")
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

    @Test("a clone selects its own helper before bundled helpers")
    func cloneSelectsPinnedHelper() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        let bin = directory.appendingPathComponent("bin")
        try FileManager.default.createDirectory(at: bin, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let helper = bin.appendingPathComponent("lf")
        try Data("#!/bin/sh\nexit 0\n".utf8).write(to: helper)
        try FileManager.default.setAttributes([.posixPermissions: 0o755], ofItemAtPath: helper.path)
        let environment = ["LF_CLONE_ROOT": directory.path, "LF_HOME": directory.path, "LF_BIN": helper.path]
        #expect(try LocalWaveAgentLauncher.controlLfPath(bundled: URL(fileURLWithPath: "/bin/sh"), environment: environment) == helper.path)
        try FileManager.default.removeItem(at: helper)
        #expect(throws: LocalLfError.self) {
            try LocalWaveAgentLauncher.controlLfPath(bundled: URL(fileURLWithPath: "/bin/sh"), environment: environment)
        }
    }

    @Test("a missing bundled helper never falls through to PATH")
    func missingBundledHelperFails() {
        #expect {
            try LocalWaveAgentLauncher.controlLfPath(
                bundled: nil
            )
        } throws: { error in
            guard error is LocalLfError else { return false }
            return error.localizedDescription.contains("missing its executable bundled lf helper")
                && error.localizedDescription.contains("PATH fallback is disabled")
        }
    }

    @Test("the app selects its bundled helper")
    func appUsesBundledHelper() throws {
        #expect(try LocalWaveAgentLauncher.controlLfPath(bundled: URL(fileURLWithPath: "/bin/sh")) == "/bin/sh")
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
