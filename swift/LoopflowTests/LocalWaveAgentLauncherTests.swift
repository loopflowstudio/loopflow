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

    @Test("canonical local and remote Homes route once")
    func canonicalHomeRouting() {
        #expect(!LaunchTargetLauncher.isRemoteHome("jack@local"))
        #expect(LaunchTargetLauncher.isRemoteHome("ssh://jack@builder.example:22"))
    }

    @Test("Ask sessions attach through their generic Run route")
    func askSessionCommands() {
        let local = AskSessionRecord(
            askId: "ask-1",
            runId: "run-1",
            homeRoute: "jack@local",
            attachArgv: ["lf", "run", "attach", "run-1"]
        )
        #expect(LaunchTargetLauncher.command(for: local, localCwd: "/repo") == .init(
            cwd: "/repo",
            argv: ["lf", "run", "attach", "run-1"],
            environment: [:]
        ))

        let remote = AskSessionRecord(
            askId: "ask-2",
            runId: "run-2",
            homeRoute: "ssh://jack@builder.example:2200",
            attachArgv: ["lf", "run", "attach", "run-2"]
        )
        #expect(LaunchTargetLauncher.command(for: remote, localCwd: "/repo") == .init(
            cwd: "/",
            argv: [
                "ssh", "-p", "2200", "jack@builder.example",
                "exec 'lf' 'run' 'attach' 'run-2'",
            ],
            environment: [:]
        ))
    }

    @Test("development launcher preserves the selected Home registry")
    func launchEnvironmentPreservesRegistry() {
        let environment = GUIProcessEnvironment.enriched([
            "PATH": "/usr/bin:/bin",
            "LF_HOME": "/tmp/loopflow-development-home",
            "LF_DB_PATH": "/tmp/loopflow-development-home/loopflow.db",
        ])

        #expect(environment["LF_HOME"] == "/tmp/loopflow-development-home")
        #expect(environment["LF_DB_PATH"] == "/tmp/loopflow-development-home/loopflow.db")
    }

    @Test("stop command uses the single-wave lifecycle verb")
    func stopCommandShape() {
        #expect(LocalWaveAgentLauncher.waveStopCommand(
            lfPath: "/Applications/Loopflow.app/Contents/MacOS/lf",
            waveName: "product"
        ) == [
            "/Applications/Loopflow.app/Contents/MacOS/lf", "stop", "product",
        ])
    }

    @Test("Task controls use the existing lifecycle verbs")
    func taskControlCommandShapes() {
        let lf = "/Applications/Loopflow.app/Contents/MacOS/lf"

        #expect(LocalWaveAgentLauncher.taskRunCommand(lfPath: lf, issue: "W2-131") == [
            lf, "task", "run", "W2-131",
        ])
        #expect(LocalWaveAgentLauncher.taskStartCommand(
            lfPath: lf,
            title: "Refine LOOPFLOW.md 5e41e69b",
            project: "context-lab",
            directive: "Refine text for LOOPFLOW.md."
        ) == [
            lf, "task", "start", "context-lab", "Refine LOOPFLOW.md 5e41e69b",
            "--directive", "Refine text for LOOPFLOW.md.",
            "--json",
        ])
        #expect(LocalWaveAgentLauncher.taskResumeCommand(lfPath: lf, issue: "W2-131") == [
            lf, "task", "resume", "W2-131",
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
    func taskStartReceiptDecodes() throws {
        let receipt = try LocalWaveAgentLauncher.taskStartReceipt("""
        {
          "issue_identifier": "W2-201",
          "project": "auditability",
          "wave": "product"
        }
        """)

        #expect(receipt == TaskStartReceipt(
            issueIdentifier: "W2-201",
            project: "auditability",
            wave: "product"
        ))
    }

    // MARK: - Bundled binary boundary

    @Test("the install-owned machine gate supplies executable and store authority")
    func installedMachineGatePrecedesBundle() throws {
        let accountHome = FileManager.default.temporaryDirectory
            .appendingPathComponent("loopflow-control-home-\(UUID().uuidString)", isDirectory: true)
        defer { try? FileManager.default.removeItem(at: accountHome) }
        let installed = accountHome
            .appendingPathComponent(".lf-machine/install/gates/1", isDirectory: true)
            .appendingPathComponent("lf")
        try FileManager.default.createDirectory(
            at: installed.deletingLastPathComponent(),
            withIntermediateDirectories: true
        )
        try Data().write(to: installed)
        try FileManager.default.setAttributes(
            [.posixPermissions: 0o755],
            ofItemAtPath: installed.path
        )

        let authority = try LocalControlAuthority.resolve(
            accountHome: accountHome,
            bundled: URL(fileURLWithPath: "/Applications/Loopflow.app/Contents/MacOS/lf")
        )

        #expect(authority == .installedMachine(executable: installed))
    }

    @Test("an account-local bin cannot replace install authority")
    func accountLocalBinCannotReplaceInstallAuthority() throws {
        let accountHome = FileManager.default.temporaryDirectory
            .appendingPathComponent("loopflow-control-home-\(UUID().uuidString)", isDirectory: true)
        defer { try? FileManager.default.removeItem(at: accountHome) }
        let unrelatedDirectory = accountHome
            .appendingPathComponent(".local", isDirectory: true)
            .appendingPathComponent("bin", isDirectory: true)
        try FileManager.default.createDirectory(
            at: unrelatedDirectory,
            withIntermediateDirectories: true
        )
        let unrelated = unrelatedDirectory.appendingPathComponent("lf")
        try Data().write(to: unrelated)
        try FileManager.default.setAttributes(
            [.posixPermissions: 0o755],
            ofItemAtPath: unrelated.path
        )
        let bundled = URL(fileURLWithPath: "/bin/sh")

        let authority = try LocalControlAuthority.resolve(
            accountHome: accountHome,
            bundled: bundled
        )

        #expect(authority == .bundledOffline(executable: bundled))
    }

    @Test("a missing machine install uses the typed bundled offline fallback")
    func missingMachineInstallUsesBundle() throws {
        let bundled = URL(fileURLWithPath: "/bin/sh")
        let authority = try LocalControlAuthority.resolve(
            accountHome: URL(fileURLWithPath: "/path/that/does/not/exist", isDirectory: true),
            bundled: bundled
        )

        #expect(authority == .bundledOffline(executable: bundled))
    }

    @Test("a broken installed gate fails instead of reading a different store")
    func brokenInstalledGateFails() throws {
        let accountHome = FileManager.default.temporaryDirectory
            .appendingPathComponent("loopflow-control-home-\(UUID().uuidString)", isDirectory: true)
        defer { try? FileManager.default.removeItem(at: accountHome) }
        let gate = accountHome
            .appendingPathComponent(".lf-machine/install/gates/1", isDirectory: true)
            .appendingPathComponent("lf")
        try FileManager.default.createDirectory(
            at: gate.deletingLastPathComponent(),
            withIntermediateDirectories: true
        )
        try Data().write(to: gate)

        #expect {
            try LocalControlAuthority.resolve(
                accountHome: accountHome,
                bundled: URL(fileURLWithPath: "/bin/sh")
            )
        } throws: { error in
            error.localizedDescription.contains("machine entry gate is not executable")
        }
    }

    @Test("a receipt with no machine gate fails instead of reading a different store")
    func missingInstalledGateFails() throws {
        let accountHome = FileManager.default.temporaryDirectory
            .appendingPathComponent("loopflow-control-home-\(UUID().uuidString)", isDirectory: true)
        defer { try? FileManager.default.removeItem(at: accountHome) }
        let installRoot = accountHome
            .appendingPathComponent(".lf-machine", isDirectory: true)
            .appendingPathComponent("install", isDirectory: true)
        try FileManager.default.createDirectory(
            at: installRoot,
            withIntermediateDirectories: true
        )
        try Data("{}".utf8).write(to: installRoot.appendingPathComponent("active.json"))

        #expect {
            try LocalControlAuthority.resolve(
                accountHome: accountHome,
                bundled: URL(fileURLWithPath: "/bin/sh")
            )
        } throws: { error in
            error.localizedDescription.contains("install receipt exists")
        }
    }

    @Test("a missing machine and bundled helper fails clearly")
    func missingControlHelpersFail() {
        #expect {
            try LocalControlAuthority.resolve(
                accountHome: URL(fileURLWithPath: "/path/that/does/not/exist", isDirectory: true),
                bundled: nil
            )
        } throws: { error in
            guard error is LocalLfError else { return false }
            return error.localizedDescription.contains("missing its executable offline lf helper")
        }
    }

    #if !SWIFT_PACKAGE
    @Test("the hosted app executes and decodes its bundled process activity")
    func hostedBundleProcessActivityDecodes() async throws {
        let helper = try #require(Bundle.main.url(forAuxiliaryExecutable: "lf"))
        #expect(FileManager.default.isExecutableFile(atPath: helper.path))
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

    @Test("start hint keeps the launch command intact as inline code")
    func startHintFormatsCommandAsCode() {
        let hint = waveStartHint(waveName: "goals")

        #expect(
            String(hint.characters)
                == "Start it here, or run lf start goals in a terminal — its conversation appears here live."
        )

        let codeRuns = hint.runs.filter { $0.inlinePresentationIntent == .code }
        #expect(codeRuns.map { String(hint.characters[$0.range]) } == ["lf start goals"])
    }
}
#endif
