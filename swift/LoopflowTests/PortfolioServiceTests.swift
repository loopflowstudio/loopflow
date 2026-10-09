import Foundation
import Testing
@testable import LoopflowMac

@Suite("Portfolio Service")
struct PortfolioServiceTests {
    @Test("addRepo keeps most recently opened repo first and de-duplicates by path")
    func addRepoDeduplicatesAndSorts() throws {
        let (defaults, key, cleanup) = makeDefaults()
        defer { cleanup() }
        let root = try makeTempDirectory(prefix: "portfolio-service")
        defer { try? FileManager.default.removeItem(at: root) }
        let repoA = try makeGitRepo("repo-a", in: root)
        let repoB = try makeGitRepo("repo-b", in: root)

        let service = PortfolioService(defaults: defaults, key: key)
        service.addRepo(repoA)
        service.addRepo(repoB)
        service.addRepo(repoA)

        #expect(service.repos.count == 2)
        #expect(service.repos.first?.path == repoA.normalizedFilePath)
        #expect(service.repos.last?.path == repoB.normalizedFilePath)
    }

    @Test("adding a linked worktree stores its main repository")
    func addRepoCollapsesLinkedWorktree() throws {
        let (defaults, key, cleanup) = makeDefaults()
        defer { cleanup() }
        let root = try makeTempDirectory(prefix: "portfolio-worktree")
        defer { try? FileManager.default.removeItem(at: root) }
        let mainRepo = try makeGitRepo("repo", in: root)
        let worktree = root.appendingPathComponent("repo.feature", isDirectory: true)
        try git(["worktree", "add", "-q", worktree.path], at: mainRepo)

        let service = PortfolioService(defaults: defaults, key: key)
        service.addRepo(worktree)

        #expect(service.repos.map(\.path) == [mainRepo.normalizedFilePath])
        #expect(service.repos.map(\.displayName) == ["repo"])
    }

    @Test("adding a non-repository leaves the portfolio unchanged")
    func addRepoRejectsNonRepository() throws {
        let (defaults, key, cleanup) = makeDefaults()
        defer { cleanup() }
        let root = try makeTempDirectory(prefix: "portfolio-non-repo")
        defer { try? FileManager.default.removeItem(at: root) }

        let service = PortfolioService(defaults: defaults, key: key)

        #expect(service.addRepo(root) == nil)
        #expect(service.repos.isEmpty)
    }

    @Test("loading stored worktrees migrates them to one main repository")
    func loadReposNormalizesStoredWorktrees() throws {
        let (defaults, key, cleanup) = makeDefaults()
        defer { cleanup() }
        let root = try makeTempDirectory(prefix: "portfolio-stored-worktree")
        defer { try? FileManager.default.removeItem(at: root) }
        let mainRepo = try makeGitRepo("repo", in: root)
        let worktree = root.appendingPathComponent("repo.feature", isDirectory: true)
        try git(["worktree", "add", "-q", worktree.path], at: mainRepo)
        let entries = [
            PortfolioRepo(path: worktree.path, lastOpened: Date()),
            PortfolioRepo(path: mainRepo.path, lastOpened: .distantPast),
        ]
        defaults.set(try JSONEncoder().encode(entries), forKey: key)

        let service = PortfolioService(defaults: defaults, key: key)

        #expect(service.repos.map(\.path) == [mainRepo.normalizedFilePath])
        let stored = try #require(defaults.data(forKey: key))
        #expect(try JSONDecoder().decode([PortfolioRepo].self, from: stored) == service.repos)
    }

    @Test("removeRepo updates stored list")
    func removeRepo() throws {
        let (defaults, key, cleanup) = makeDefaults()
        defer { cleanup() }
        let root = try makeTempDirectory(prefix: "portfolio-remove")
        defer { try? FileManager.default.removeItem(at: root) }
        let repoA = try makeGitRepo("repo-a", in: root)
        let repoB = try makeGitRepo("repo-b", in: root)

        let service = PortfolioService(defaults: defaults, key: key)
        service.addRepo(repoA)
        service.addRepo(repoB)

        service.removeRepo(repoA)
        #expect(service.repos.count == 1)
        #expect(service.repos[0].path == repoB.normalizedFilePath)
    }

    private func makeDefaults() -> (UserDefaults, String, () -> Void) {
        let suiteName = "portfolio-tests-\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: suiteName)!
        let key = "portfolio.repos"
        return (defaults, key, { defaults.removePersistentDomain(forName: suiteName) })
    }

    private func makeGitRepo(_ name: String, in root: URL) throws -> URL {
        let repo = root.appendingPathComponent(name, isDirectory: true)
        try FileManager.default.createDirectory(at: repo, withIntermediateDirectories: true)
        try git(["init", "-q"], at: repo)
        try git(
            [
                "-c", "user.email=t@t", "-c", "user.name=t",
                "commit", "-q", "--allow-empty", "-m", "init",
            ],
            at: repo
        )
        return repo
    }

    private func git(_ args: [String], at directory: URL) throws {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/env")
        process.arguments = ["git", "-C", directory.path] + args
        process.standardOutput = Pipe()
        process.standardError = Pipe()
        try process.run()
        process.waitUntilExit()
        try #require(process.terminationStatus == 0, "git \(args.joined(separator: " "))")
    }

    private func makeTempDirectory(prefix: String) throws -> URL {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("\(prefix)-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        return directory
    }
}
