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

    // MARK: - Binary resolution + capability probe

    @Test("candidate order: bundled, PATH hits in order, dev-tree build last")
    func candidateOrder() {
        let candidates = LocalWaveAgentLauncher.lfCandidates(
            originRepo: "/Users/jack/src/loopflow",
            bundled: URL(fileURLWithPath: "/Applications/Concerto.app/Contents/MacOS/lf"),
            pathEnv: "/missing/bin:/opt/homebrew/bin:/usr/local/bin",
            isExecutableFile: { $0 != "/missing/bin/lf" }
        )

        #expect(candidates == [
            "/Applications/Concerto.app/Contents/MacOS/lf",
            "/opt/homebrew/bin/lf",
            "/usr/local/bin/lf",
            "/Users/jack/src/loopflow/target/release/lf",
        ])
    }

    @Test("probe passes: the first candidate wins")
    func probePassesFirstCandidate() throws {
        let resolved = try LocalWaveAgentLauncher.resolveWaveCapableLf(
            originRepo: "/Users/jack/src/loopflow",
            bundled: URL(fileURLWithPath: "/Applications/Concerto.app/Contents/MacOS/lf"),
            pathEnv: "/usr/local/bin",
            isExecutableFile: { _ in true },
            probe: { _ in true }
        )

        #expect(resolved == "/Applications/Concerto.app/Contents/MacOS/lf")
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
            }
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
                probe: { _ in false }
            )
        } throws: { error in
            guard case let WaveLaunchError.noUsableLf(detail) = error else { return false }
            return detail.contains("/Users/jack/.local/bin/lf")
                && detail.contains("/Users/jack/src/loopflow/target/release/lf")
                && detail.contains("lf help wave")
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
                probe: { _ in true }
            )
        } throws: { error in
            guard case let WaveLaunchError.noUsableLf(detail) = error else { return false }
            return detail.contains("not bundled") && detail.contains("not on PATH")
        }
    }

    // MARK: - Double-launch guard

    @Test("a live endpoint blocks the launch")
    func endpointBlocksLaunch() {
        let action = LocalWaveAgentLauncher.launchAction(
            sessionName: "lf-loopflow-goals",
            sessionExists: false,
            endpoint: "127.0.0.1:52340"
        )

        #expect(action == .blocked("Wave already has a live server at 127.0.0.1:52340."))
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
        #expect(LocalWaveAgentLauncher.launchAction(
            sessionName: "lf-loopflow-goals",
            sessionExists: false,
            endpoint: live
        ) == .launch, "stale pointer: clear to launch")
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

    @Test("an existing tmux session without a live server blocks with reset guidance")
    func tmuxSessionWithoutEndpointBlocks() {
        let action = LocalWaveAgentLauncher.launchAction(
            sessionName: "lf-loopflow-goals",
            sessionExists: true,
            endpoint: nil
        )

        guard case let .blocked(reason) = action else {
            Issue.record("expected blocked action")
            return
        }
        #expect(reason.contains("lf-loopflow-goals"))
        #expect(reason.contains("lf op reset-waves"))
    }

    @Test("awaitLiveEndpoint returns when a session publishes on a later probe")
    func awaitLiveEndpointSeesLatePublisher() {
        var reads = 0
        var sleeps: [TimeInterval] = []

        let endpoint = LocalWaveAgentLauncher.awaitLiveEndpoint(
            origin: "/repo",
            waveName: "goals",
            attempts: 3,
            readRecorded: { origin, wave in
                #expect(origin == "/repo")
                #expect(wave == "goals")
                reads += 1
                return reads == 2 ? "127.0.0.1:52340" : nil
            },
            probe: { endpoint in
                #expect(endpoint == "127.0.0.1:52340")
                return "goals"
            },
            sleep: { sleeps.append($0) }
        )

        #expect(endpoint == "127.0.0.1:52340")
        #expect(reads == 2)
        #expect(sleeps == [1])
    }

    @Test("awaitLiveEndpoint does one probe when only one attempt is requested")
    func awaitLiveEndpointSingleAttemptDoesNotSleep() {
        var reads = 0
        var sleeps: [TimeInterval] = []

        let endpoint = LocalWaveAgentLauncher.awaitLiveEndpoint(
            origin: "/repo",
            waveName: "goals",
            attempts: 1,
            readRecorded: { _, _ in
                reads += 1
                return nil
            },
            probe: { _ in
                Issue.record("must not probe without a recorded endpoint")
                return nil
            },
            sleep: { sleeps.append($0) }
        )

        #expect(endpoint == nil)
        #expect(reads == 1)
        #expect(sleeps.isEmpty)
    }

    @Test("no session and no endpoint: clear to launch")
    func clearToLaunch() {
        let action = LocalWaveAgentLauncher.launchAction(
            sessionName: "lf-loopflow-goals",
            sessionExists: false,
            endpoint: nil
        )

        #expect(action == .launch)
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
