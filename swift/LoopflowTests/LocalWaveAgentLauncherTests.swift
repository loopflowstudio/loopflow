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

    private func loadFixtureData(_ name: String, sourceFile: String = #filePath) throws -> Data {
        let testFile = URL(fileURLWithPath: sourceFile)
        let fixtures = testFile
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/dto")
            .appendingPathComponent(name)
        return try Data(contentsOf: fixtures)
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

    // MARK: - Binary resolution + capability probe

    @Test("candidate order: bundled, PATH hits in order, dev-tree build last")
    func candidateOrder() {
        let candidates = LocalWaveAgentLauncher.lfCandidates(
            originRepo: "/Users/jack/src/loopflow",
            bundled: URL(fileURLWithPath: "/Applications/Loopflow.app/Contents/MacOS/lf"),
            pathEnv: "/missing/bin:/opt/homebrew/bin:/usr/local/bin",
            isExecutableFile: { $0 != "/missing/bin/lf" }
        )

        #expect(candidates == [
            "/Applications/Loopflow.app/Contents/MacOS/lf",
            "/opt/homebrew/bin/lf",
            "/usr/local/bin/lf",
            "/Users/jack/src/loopflow/target/release/lf",
        ])
    }

    @Test("probe passes: the first candidate wins")
    func probePassesFirstCandidate() throws {
        let resolved = try LocalWaveAgentLauncher.resolveWaveCapableLf(
            originRepo: "/Users/jack/src/loopflow",
            bundled: URL(fileURLWithPath: "/Applications/Loopflow.app/Contents/MacOS/lf"),
            pathEnv: "/usr/local/bin",
            isExecutableFile: { _ in true },
            probe: { _ in true },
            useCache: false
        )

        #expect(resolved == "/Applications/Loopflow.app/Contents/MacOS/lf")
    }

    @Test("a stale PATH lf is rejected by the probe; the dev-tree build wins")
    func staleLfFallsThroughToDevBuild() throws {
        var probed: [String] = []
        let resolved = try LocalWaveAgentLauncher.resolveWaveCapableLf(
            originRepo: "/Users/jack/src/loopflow",
            bundled: nil,
            pathEnv: "/Users/jack/.local/bin",
            isExecutableFile: { _ in true },
            probe: { candidate in
                probed.append(candidate)
                return candidate == "/Users/jack/src/loopflow/target/release/lf"
            },
            useCache: false
        )

        #expect(resolved == "/Users/jack/src/loopflow/target/release/lf")
        #expect(probed == [
            "/Users/jack/.local/bin/lf",
            "/Users/jack/src/loopflow/target/release/lf",
        ], "walks candidates in order, probing each")
    }

    @Test("every candidate rejected: the error names what was found and why")
    func allCandidatesRejected() {
        #expect {
            try LocalWaveAgentLauncher.resolveWaveCapableLf(
                originRepo: "/Users/jack/src/loopflow",
                bundled: nil,
                pathEnv: "/Users/jack/.local/bin",
                isExecutableFile: { _ in true },
                probe: { _ in false },
                useCache: false
            )
        } throws: { error in
            guard case let WaveLaunchError.noUsableLf(detail) = error else { return false }
            return detail.contains("/Users/jack/.local/bin/lf")
                && detail.contains("/Users/jack/src/loopflow/target/release/lf")
                && detail.contains("lf help start")
                && detail.contains("lf help stop")
                && detail.contains("lf help pause")
                && detail.contains("lf help resume")
        }
    }

    @Test("nothing bundled, nothing on PATH, no dev build: a clear error")
    func nothingResolves() {
        #expect {
            try LocalWaveAgentLauncher.resolveWaveCapableLf(
                originRepo: "/Users/jack/src/loopflow",
                bundled: nil,
                pathEnv: "/usr/local/bin:/usr/bin",
                isExecutableFile: { _ in false },
                probe: { _ in true },
                useCache: false
            )
        } throws: { error in
            guard case let WaveLaunchError.noUsableLf(detail) = error else { return false }
            return detail.contains("not bundled") && detail.contains("not on PATH")
        }
    }

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
