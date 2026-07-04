// The wave launcher's pure parts: how the `lf` binary resolves, the exact
// detached-tmux invocation, the double-launch guard, and the not-running copy.

#if os(macOS)
import Foundation
import Testing
@testable import Concerto

@Suite("Local wave launcher")
struct LocalWaveAgentLauncherTests {
    // MARK: - Command construction

    @Test("launch command is a detached tmux session at the wave's repo")
    func launchCommandShape() {
        let args = LocalWaveAgentLauncher.waveLaunchCommand(
            lfPath: "/Applications/Concerto.app/Contents/MacOS/lf",
            sessionName: "lf-loopflow-goals",
            repoPath: "/Users/jack/src/loopflow",
            waveName: "goals"
        )

        #expect(args == [
            "tmux", "new-session", "-d",
            "-s", "lf-loopflow-goals",
            "-c", "/Users/jack/src/loopflow",
            "/Applications/Concerto.app/Contents/MacOS/lf", "wave", "goals",
        ])
    }

    // MARK: - Binary resolution

    @Test("bundled lf wins over PATH")
    func bundledBinaryWins() {
        let resolved = LocalWaveAgentLauncher.resolveLfBinary(
            bundled: URL(fileURLWithPath: "/Applications/Concerto.app/Contents/MacOS/lf"),
            pathEnv: "/usr/local/bin:/usr/bin",
            isExecutableFile: { _ in true }
        )

        #expect(resolved == "/Applications/Concerto.app/Contents/MacOS/lf")
    }

    @Test("PATH lookup walks directories in order")
    func pathLookupInOrder() {
        let resolved = LocalWaveAgentLauncher.resolveLfBinary(
            bundled: nil,
            pathEnv: "/missing/bin:/opt/homebrew/bin:/usr/local/bin",
            isExecutableFile: { $0 != "/missing/bin/lf" }
        )

        #expect(resolved == "/opt/homebrew/bin/lf")
    }

    @Test("nothing bundled, nothing on PATH: nil")
    func nothingResolves() {
        let resolved = LocalWaveAgentLauncher.resolveLfBinary(
            bundled: nil,
            pathEnv: "/usr/local/bin:/usr/bin",
            isExecutableFile: { _ in false }
        )

        #expect(resolved == nil)
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
