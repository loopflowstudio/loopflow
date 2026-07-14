import Foundation
import Testing
@testable import LoopflowMac
@testable import Loopflow

@MainActor
@Suite("Portfolio Repo State")
struct PortfolioRepoStateTests {
    @Test("connected waves keep only this repo's rows")
    func connectedWavesScopedToRepo() {
        let repoURL = URL(fileURLWithPath: "/tmp/portfolio-scope")
        let repo = PortfolioRepo(path: repoURL.normalizedFilePath, lastOpened: Date())
        let state = PortfolioRepoState(repo: repo)

        let mine = makeWave(id: "mine", repoPath: repo.path, status: .running)
        let other = makeWave(
            id: "other",
            repoPath: URL(fileURLWithPath: "/tmp/portfolio-other").normalizedFilePath,
            status: .running
        )
        state.applyConnectedWaves([mine, other])

        #expect(state.waves.map(\.id) == ["mine"])
    }

    @Test("waves sort by presence before name")
    func wavesSortByPresenceBeforeName() {
        let repoURL = URL(fileURLWithPath: "/tmp/portfolio-priority")
        let repo = PortfolioRepo(path: repoURL.normalizedFilePath, lastOpened: Date())
        let state = PortfolioRepoState(repo: repo)

        state.applyConnectedWaves([
            makeWave(id: "running-b", repoPath: repo.path, status: .running),
            makeWave(id: "idle", repoPath: repo.path, status: .idle),
            makeWave(id: "paused", repoPath: repo.path, status: .paused),
            makeWave(id: "running-a", repoPath: repo.path, status: .running),
        ])

        #expect(state.waves.map(\.id) == ["running-a", "running-b", "paused", "idle"])
    }

    @Test("wave agent session name mirrors lf tmux handle")
    func waveAgentSessionNameMirrorsLfTmuxHandle() {
        let name = PortfolioRepoState.waveAgentSessionName(
            repoPath: "/Users/jack/src/loopflow",
            waveName: "loopflow"
        )

        #expect(name == "lf-loopflow-loopflow")
    }

    @Test("a worktree path names the same tmux session the launcher creates")
    func worktreeProbeNameMatchesLaunchName() throws {
        // The launcher resolves a worktree to its origin before naming the
        // session; the rail's status probe and the attach hint must land on
        // that same name from the raw worktree path — otherwise a running
        // wave shows idle in a worktree rail. Real git, like WaveOriginTests:
        // the whole point is the origin resolution.
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("session-name-\(UUID().uuidString)", isDirectory: true)
        let origin = root.appendingPathComponent("repo", isDirectory: true)
        let worktree = root.appendingPathComponent("repo.wt", isDirectory: true)
        try FileManager.default.createDirectory(at: origin, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root) }

        func git(_ args: [String], at dir: URL) throws {
            let process = Process()
            process.executableURL = URL(fileURLWithPath: "/usr/bin/env")
            process.arguments = ["git", "-C", dir.path] + args
            process.standardOutput = Pipe()
            process.standardError = Pipe()
            try process.run()
            process.waitUntilExit()
            try #require(process.terminationStatus == 0, "git \(args.joined(separator: " "))")
        }
        try git(["init", "-q"], at: origin)
        try git(
            ["-c", "user.email=t@t", "-c", "user.name=t",
             "commit", "--allow-empty", "-q", "-m", "init"],
            at: origin
        )
        try git(["worktree", "add", "-q", worktree.path], at: origin)

        let probeName = PortfolioRepoState.waveAgentSessionName(
            repoPath: worktree.path, waveName: "goals"
        )
        let launchName = PortfolioRepoState.waveAgentSessionName(
            repoPath: WaveOrigin.resolve(worktree.path), waveName: "goals"
        )
        #expect(probeName == launchName)
        #expect(probeName == "lf-repo-goals", "named after the origin, not the worktree")

        let repo = PortfolioRepo(path: worktree.path.normalizedFilePath, lastOpened: Date())
        let state = PortfolioRepoState(repo: repo)
        state.applyConnectedWaves([
            makeWave(id: "goals", repoPath: origin.path.normalizedFilePath, status: .running),
        ])
        #expect(state.waves.map(\.id) == ["goals"], "a dev worktree sees its origin's registry row")
    }

    @Test("createWave writes GOAL.md at the origin repo, not the worktree")
    func createWaveWritesAtOrigin() async throws {
        // Authoring from a worktree rail must land wave/<name>/GOAL.md at the
        // ORIGIN — every reader (endpoint discovery, launcher, probe) resolves
        // the origin, so a worktree-local GOAL.md is a wave nobody finds.
        // Real git, like WaveOriginTests: the origin resolution is the point.
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("create-wave-\(UUID().uuidString)", isDirectory: true)
        let origin = root.appendingPathComponent("repo", isDirectory: true)
        let worktree = root.appendingPathComponent("repo.wt", isDirectory: true)
        try FileManager.default.createDirectory(at: origin, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root) }

        func git(_ args: [String], at dir: URL) throws {
            let process = Process()
            process.executableURL = URL(fileURLWithPath: "/usr/bin/env")
            process.arguments = ["git", "-C", dir.path] + args
            process.standardOutput = Pipe()
            process.standardError = Pipe()
            try process.run()
            process.waitUntilExit()
            try #require(process.terminationStatus == 0, "git \(args.joined(separator: " "))")
        }
        try git(["init", "-q"], at: origin)
        try git(
            ["-c", "user.email=t@t", "-c", "user.name=t",
             "commit", "--allow-empty", "-q", "-m", "init"],
            at: origin
        )
        try git(["worktree", "add", "-q", worktree.path], at: origin)

        let repo = PortfolioRepo(path: worktree.path.normalizedFilePath, lastOpened: Date())
        let state = PortfolioRepoState(repo: repo)
        try await state.createWave(name: "goals")

        let originGoal = origin.appendingPathComponent("wave/goals/GOAL.md")
        let worktreeGoal = worktree.appendingPathComponent("wave/goals/GOAL.md")
        #expect(FileManager.default.fileExists(atPath: originGoal.path), "GOAL.md lands at the origin")
        #expect(FileManager.default.fileExists(atPath: origin.appendingPathComponent("wave/goals/MEMORY.md").path))
        #expect(!FileManager.default.fileExists(atPath: worktreeGoal.path), "nothing written into the worktree")
    }

    private func makeWave(id: String, repoPath: String, status: WaveStatus) -> Wave {
        Wave(
            id: id,
            name: id,
            repo: repoPath,
            status: status
        )
    }
}
