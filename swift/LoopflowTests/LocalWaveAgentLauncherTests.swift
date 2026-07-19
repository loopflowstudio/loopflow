// The wave launcher's pure parts: how the `lf` binary resolves, the exact
// detached-tmux invocation, the double-launch guard, and the not-running copy.

#if os(macOS)
import Foundation
import Testing
@testable import LoopflowMac

@Suite("Local wave launcher")
struct LocalWaveAgentLauncherTests {
    // MARK: - Command construction

    @Test("canonical local and remote Homes route once")
    func canonicalHomeRouting() {
        #expect(!LaunchTargetLauncher.isRemoteHome("jack@local"))
        #expect(LaunchTargetLauncher.isRemoteHome("ssh://jack@builder.example:22"))
    }

    @Test("launch command is a detached tmux session at the wave's repo")
    func launchCommandShape() {
        let args = LocalWaveAgentLauncher.waveLaunchCommand(
            lfPath: "/Applications/Loopflow.app/Contents/MacOS/lf",
            sessionName: "lf-loopflow-goals",
            repoPath: "/Users/jack/src/loopflow",
            waveName: "goals",
            environment: [:]
        )

        #expect(args == [
            "tmux", "new-session", "-d",
            "-s", "lf-loopflow-goals",
            "-c", "/Users/jack/src/loopflow",
            "/Applications/Loopflow.app/Contents/MacOS/lf", "wave", "goals",
        ])
    }

    @Test("launch carries an explicit registry through tmux")
    func launchCommandForwardsRegistryEnvironment() {
        let args = LocalWaveAgentLauncher.waveLaunchCommand(
            lfPath: "/Applications/Loopflow.app/Contents/MacOS/lf",
            sessionName: "lf-loopflow-goals",
            repoPath: "/Users/jack/src/loopflow",
            waveName: "goals",
            environment: [
                "LF_DB_PATH": "/Users/jack/.lf/demo.db",
                "UNRELATED": "ignored",
            ]
        )

        #expect(args == [
            "tmux", "new-session", "-d",
            "-s", "lf-loopflow-goals",
            "-c", "/Users/jack/src/loopflow",
            "/usr/bin/env", "LF_DB_PATH=/Users/jack/.lf/demo.db",
            "/Applications/Loopflow.app/Contents/MacOS/lf", "wave", "goals",
        ])
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
            lf, "task", "start", "Refine LOOPFLOW.md 5e41e69b",
            "--project", "context-lab",
            "--directive", "Refine text for LOOPFLOW.md.",
            "--json",
        ])
        #expect(LocalWaveAgentLauncher.taskResumeCommand(lfPath: lf, issue: "W2-131") == [
            lf, "task", "resume", "W2-131",
        ])
        #expect(LocalWaveAgentLauncher.taskInterruptCommand(lfPath: lf, issue: "W2-131") == [
            lf, "task", "interrupt", "W2-131",
        ])
        #expect(LocalWaveAgentLauncher.taskFeedbackCommand(lfPath: lf, taskId: "task_131") == [
            lf, "work", "feedback", "task", "task_131", "--continue-on-exit",
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
                && detail.contains("lf help wave")
                && detail.contains("lf help stop")
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

    // MARK: - Double-launch guard

    @Test("a live endpoint blocks the launch")
    func endpointBlocksLaunch() {
        let reason = LocalWaveAgentLauncher.launchBlockReason(
            sessionName: "lf-loopflow-goals",
            sessionExists: false,
            endpoint: "127.0.0.1:52340"
        )

        #expect(reason == "Wave already has a live server at 127.0.0.1:52340.")
    }

    // MARK: - Endpoint liveness probe

    @Test("a probed endpoint answering for this wave is live")
    func probedEndpointIsLive() {
        let live = LocalWaveAgentLauncher.liveEndpoint(
            recorded: "127.0.0.1:52340",
            waveName: "goals",
            probe: { endpoint in
                #expect(endpoint == "127.0.0.1:52340")
                return "goals"
            }
        )

        #expect(live == "127.0.0.1:52340")
    }

    @Test("a dead endpoint is stale: the pointer file alone never blocks")
    func deadEndpointIsStale() {
        // SIGKILL / power loss leaves `.wave-endpoint` behind; a probe that
        // gets no answer must clear the launch, mirroring Rust live_endpoint.
        let live = LocalWaveAgentLauncher.liveEndpoint(
            recorded: "127.0.0.1:52340",
            waveName: "goals",
            probe: { _ in nil }
        )

        #expect(live == nil)
        #expect(LocalWaveAgentLauncher.launchBlockReason(
            sessionName: "lf-loopflow-goals",
            sessionExists: false,
            endpoint: live
        ) == nil, "stale pointer: clear to launch")
    }

    @Test("a server answering for a different wave is stale")
    func mismatchedWaveIsStale() {
        let live = LocalWaveAgentLauncher.liveEndpoint(
            recorded: "127.0.0.1:52340",
            waveName: "goals",
            probe: { _ in "ship" }
        )

        #expect(live == nil)
    }

    @Test("no pointer file: nothing to probe, clear to launch")
    func missingPointerNeverProbes() {
        let live = LocalWaveAgentLauncher.liveEndpoint(
            recorded: nil,
            waveName: "goals",
            probe: { _ in
                Issue.record("must not probe without a recorded endpoint")
                return nil
            }
        )

        #expect(live == nil)
    }

    @Test("an existing tmux session blocks the launch")
    func tmuxSessionBlocksLaunch() {
        let reason = LocalWaveAgentLauncher.launchBlockReason(
            sessionName: "lf-loopflow-goals",
            sessionExists: true,
            endpoint: nil
        )

        #expect(reason?.contains("lf-loopflow-goals") == true)
    }

    @Test("no session and no endpoint: clear to launch")
    func clearToLaunch() {
        let reason = LocalWaveAgentLauncher.launchBlockReason(
            sessionName: "lf-loopflow-goals",
            sessionExists: false,
            endpoint: nil
        )

        #expect(reason == nil)
    }

    // MARK: - Not-running copy

    @Test("start hint keeps the launch command intact as inline code")
    func startHintFormatsCommandAsCode() {
        let hint = waveStartHint(waveName: "goals")

        #expect(
            String(hint.characters)
                == "Start it here, or run lf wave goals in a terminal — its conversation appears here live."
        )

        let codeRuns = hint.runs.filter { $0.inlinePresentationIntent == .code }
        #expect(codeRuns.map { String(hint.characters[$0.range]) } == ["lf wave goals"])
    }
}
#endif
