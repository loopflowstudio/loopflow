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

    @Test("development launcher preserves the selected Home registry")
    func launchEnvironmentPreservesRegistry() {
        let environment = GUIProcessEnvironment.enriched([
            "PATH": "/usr/bin:/bin",
            "LF_HOME": "/tmp/loopflow-development-home",
            "LF_WAVE_ID": "launching-wave",
            "LF_FLOW_STEP": "launching-step",
            "LF_RUN_ID": "launching-run",
            "LF_WORK_ADVANCE_CLAIM": "launching-task-claim",
        ])

        #expect(environment["LF_HOME"] == "/tmp/loopflow-development-home")
        #expect(environment["LF_WAVE_ID"] == nil)
        #expect(environment["LF_FLOW_STEP"] == nil)
        #expect(environment["LF_RUN_ID"] == nil)
        #expect(environment["LF_WORK_ADVANCE_CLAIM"] == nil)
    }


    @Test("Task controls use the bounded worker commands")
    func taskControlCommandShapes() {
        let lf = "/Applications/Loopflow.app/Contents/MacOS/lf"

        #expect(LocalWaveAgentLauncher.taskRunCommand(lfPath: lf, issue: "W2-131") == [
            lf, "--task", "W2-131", "flow", "start",
        ])
        #expect(LocalWaveAgentLauncher.taskCreateCommand(
            lfPath: lf,
            title: "Refine LOOPFLOW.md 5e41e69b",
            wave: "context-lab",
            directive: "Refine text for LOOPFLOW.md."
        ) == [
            lf, "task", "create", "--run", "--wave", "context-lab", "--title", "Refine LOOPFLOW.md 5e41e69b",
            "--notes", "Refine text for LOOPFLOW.md.",
            "--json",
        ])
        #expect(LocalWaveAgentLauncher.taskRunCommand(lfPath: lf, issue: "W2-131") == [
            lf, "--task", "W2-131", "flow", "start",
        ])
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

    @Test("Task start uses the exact CLI receipt as workspace identity")
    func taskCreateReceiptDecodes() throws {
        let receipt = try LocalWaveAgentLauncher.taskCreateReceipt("""
        {
          "issue_identifier": "W2-201",
          "project": "auditability",
          "wave": "product"
        }
        """)

        #expect(receipt == TaskCreateReceipt(
            issueIdentifier: "W2-201",
            project: "auditability",
            wave: "product"
        ))
    }

    @Test("Prepared checkout receipts retain the owning Home and require its evidence")
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
        {"home_id": "home_00000000000000000000000000000001", "id": "task_1", "issue": "LOO-291", "worktree": "/src/loopflow.main-view-task", "agent": "claude"}
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
    @Test("the hosted app executes and decodes its bundled process activity")
    func hostedBundleProcessActivityDecodes() async throws {
        let helper = try LocalWaveAgentLauncher.controlLfPath()
        #expect(FileManager.default.isExecutableFile(atPath: helper))
        let query = RegistryQuery { args, cwd in
            #expect(args == ["ps", "--json"])
            #expect(cwd == nil)
            return try LocalWaveAgentLauncher.queryLf(args, cwd: cwd)
        }

        let snapshot = try await query.processActivity()
        #expect(snapshot.schemaVersion == 1)
        #expect(snapshot.observedAt > 0)
    }
    #endif

    // MARK: - Not-running copy

}

#endif
