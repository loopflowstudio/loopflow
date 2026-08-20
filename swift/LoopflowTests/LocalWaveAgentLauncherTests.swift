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

    @Test("Ask presentation offers only targets that attach the exact Invocation")
    func askPresentationFiltersWorktreeOnlyTargets() {
        let capability = LaunchTargetCapability(
            installedApps: [.warp, .vscode],
            workspaceProven: true,
            warpCommandBearing: false,
            isRemoteHome: false,
            providerIsClaude: false,
            providerSessionKnown: false
        )

        #expect(capability.offeredOptions.map(\.surface) == [.ghostty, .warp, .vscode])
        #expect(capability.attachOptions.map(\.surface) == [.ghostty])
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

    @Test("a missing bundled helper never falls through to PATH")
    func missingBundledHelperFails() {
        #expect {
            try LocalWaveAgentLauncher.bundledLfPath(bundled: nil)
        } throws: { error in
            guard error is LocalLfError else { return false }
            return error.localizedDescription.contains("missing its executable bundled lf helper")
                && error.localizedDescription.contains("PATH fallback is disabled")
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
        #expect(snapshot.usage.windows == [5, 300, 3_600, 86_400])
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
