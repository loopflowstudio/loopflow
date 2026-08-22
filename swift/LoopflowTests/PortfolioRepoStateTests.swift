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

        let mine = makeWave(
            id: "mine",
            repoPath: repo.path,
            status: .running(runID: "run_mine")
        )
        let other = makeWave(
            id: "other",
            repoPath: URL(fileURLWithPath: "/tmp/portfolio-other").normalizedFilePath,
            status: .running(runID: "run_other")
        )
        state.applyConnectedWaves([mine, other])

        #expect(state.waves.map(\.id) == ["mine"])
    }

    @Test("waves hold one stable alphabetical order, not a status regrouping")
    func wavesSortStableAlphabetical() {
        let repoURL = URL(fileURLWithPath: "/tmp/portfolio-priority")
        let repo = PortfolioRepo(path: repoURL.normalizedFilePath, lastOpened: Date())
        let state = PortfolioRepoState(repo: repo)

        // Status varies, but the row's lens carries state — so the list stays in
        // one alphabetical order and never reorders as processes start and stop.
        state.applyConnectedWaves([
            makeWave(id: "running-b", repoPath: repo.path, status: .running(runID: "run_b")),
            makeWave(id: "idle", repoPath: repo.path, status: .ready),
            makeWave(id: "paused", repoPath: repo.path, status: .done),
            makeWave(id: "running-a", repoPath: repo.path, status: .running(runID: "run_a")),
        ])

        #expect(state.waves.map(\.id) == ["idle", "paused", "running-a", "running-b"])
    }

    @Test("a development worktree sees the origin registry rows")
    func worktreeSeesOriginRegistryRows() async throws {
        // Real git, like WaveOriginTests: the origin resolution is the point.
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("registry-origin-\(UUID().uuidString)", isDirectory: true)
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
        let originPath = origin.path.normalizedFilePath
        let json = """
        [{
          "id":"goals",
          "name":"goals",
          "status":{"running":{"run_id":"run_00000000000000000000000000000001"}},
          "current":{"state":"working","reason":"working","owner":"work","controls":["attach","steer","interrupt","stop"],"progress_age_secs":0,"deadline_in_secs":1800,"step":null,"liveness":{"state":"present","observed_at":"1970-01-01T00:00:00Z","fresh":true}},
          "goal":"ship goals",
          "repo":"\(originPath)",
          "active_tasks":0,
          "active_projects":0,
          "live":true,
          "paused":false,
          "enabled":true,
          "endpoint":"127.0.0.1:5678",
          "created_at":null,
          "parent_wave_id":null,
          "retired_at":null,
          "superseded_by_wave_id":null,
          "retirement_reason":null,
          "home":{"id":"home_00000000000000000000000000000001","route":"local","created_at":"1970-01-01T00:00:00Z","observed_at":"1970-01-01T00:00:00Z"}
        }]
        """
        let query = RegistryQuery { args, _ in
            #expect(args == ["ls", "--all", "--json"])
            return json
        }
        let state = PortfolioRepoState(repo: repo, registryQuery: query)

        await state.refresh()

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

    private func makeWave(id: String, repoPath: String, status: WorkStatus) -> Wave {
        Wave(
            id: id,
            name: id,
            repo: repoPath,
            status: status
        )
    }
}
