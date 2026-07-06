import Foundation
import Testing
@testable import Concerto
@testable import LoopflowCore

@MainActor
@Suite("Portfolio Repo State")
struct PortfolioRepoStateTests {
    @Test("summary metrics count blocked and diff totals")
    func summaryMetrics() {
        let repoURL = URL(fileURLWithPath: "/tmp/portfolio-state")
        let repo = PortfolioRepo(path: repoURL.normalizedFilePath, lastOpened: Date())
        let state = PortfolioRepoState(repo: repo, connection: .local, token: nil)

        state.applyConnectedWaves([
            makeWave(id: "running", repoPath: repo.path, status: .running, diffStat: " 1 files changed, 8 insertions(+), 2 deletions(-)"),
            makeWave(id: "waiting", repoPath: repo.path, status: .waiting, diffStat: " 2 files changed, 3 insertions(+), 7 deletions(-)"),
            makeWave(id: "failed", repoPath: repo.path, status: .failed, diffStat: " 1 files changed, 4 insertions(+), 0 deletions(-)"),
        ])

        #expect(state.blockedCount == 1)
        #expect(state.totalDiff.insertions == 15)
        #expect(state.totalDiff.deletions == 9)
    }

    @Test("connected waves keep only this repo's rows")
    func connectedWavesScopedToRepo() {
        let repoURL = URL(fileURLWithPath: "/tmp/portfolio-scope")
        let repo = PortfolioRepo(path: repoURL.normalizedFilePath, lastOpened: Date())
        let state = PortfolioRepoState(repo: repo, connection: .local, token: nil)

        let mine = makeWave(id: "mine", repoPath: repo.path, status: .running, diffStat: nil)
        let other = makeWave(
            id: "other",
            repoPath: URL(fileURLWithPath: "/tmp/portfolio-other").normalizedFilePath,
            status: .running,
            diffStat: nil
        )
        state.applyConnectedWaves([mine, other])

        #expect(state.waves.map(\.id) == ["mine"])
    }

    @Test("wave agent session name mirrors lf tmux handle")
    func waveAgentSessionNameMirrorsLfTmuxHandle() {
        let name = PortfolioRepoState.waveAgentSessionName(
            repoPath: "/Users/jack/src/loopflow",
            waveName: "concerto"
        )

        #expect(name == "lf-loopflow-concerto")
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
        let state = PortfolioRepoState(repo: repo, connection: .local, token: nil)
        try await state.createWave(name: "goals")

        let originGoal = origin.appendingPathComponent("wave/goals/GOAL.md")
        let worktreeGoal = worktree.appendingPathComponent("wave/goals/GOAL.md")
        #expect(FileManager.default.fileExists(atPath: originGoal.path), "GOAL.md lands at the origin")
        #expect(FileManager.default.fileExists(atPath: origin.appendingPathComponent("wave/goals/MEMORY.md").path))
        #expect(!FileManager.default.fileExists(atPath: worktreeGoal.path), "nothing written into the worktree")
    }

    private func makeWave(id: String, repoPath: String, status: WaveStatus, diffStat: String?) -> Wave {
        Wave(
            id: id,
            name: id,
            repo: repoPath,
            flow: "build",
            direction: [],
            area: ["."],
            triggers: [],
            status: status,
            iteration: 0,
            diffStat: diffStat
        )
    }
}
