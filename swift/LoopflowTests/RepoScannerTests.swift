import Foundation
import Testing
@testable import LoopflowMac

@Suite("Repo Scanner")
struct RepoScannerTests {
    @Test("scanMainWorktrees returns Git main repos and skips linked worktrees")
    func scanFiltersRepos() throws {
        let root = try makeTempDirectory(prefix: "repo-scanner")
        defer { try? FileManager.default.removeItem(at: root) }

        let mainRepo = try makeGitRepo("main-repo", in: root)
        let linkedWorktree = root.appendingPathComponent("linked-worktree", isDirectory: true)
        try git(["worktree", "add", "-q", linkedWorktree.path], at: mainRepo)
        let nonRepo = root.appendingPathComponent("docs", isDirectory: true)
        try FileManager.default.createDirectory(at: nonRepo, withIntermediateDirectories: true)

        let foundPaths = Set(
            RepoScanner().scanMainWorktrees(in: root).map(\.normalizedFilePath)
        )

        #expect(foundPaths == [mainRepo.normalizedFilePath])
    }

    @Test("Git metadata, not dotted names, determines repository identity")
    func scanUsesGitIdentity() throws {
        let root = try makeTempDirectory(prefix: "repo-scanner-identity")
        defer { try? FileManager.default.removeItem(at: root) }

        let loopflow = try makeGitRepo("loopflow", in: root)
        _ = try makeGitRepo("loopflow.goalreview", in: root)
        _ = try makeGitRepo("my.tool", in: root)
        let linkedWorktree = root.appendingPathComponent("loopflow.feature", isDirectory: true)
        try git(["worktree", "add", "-q", linkedWorktree.path], at: loopflow)

        let names = Set(RepoScanner().scanMainWorktrees(in: root).map(\.lastPathComponent))

        #expect(names == ["loopflow", "loopflow.goalreview", "my.tool"])
        #expect(
            RepoScanner().mainRepository(linkedWorktree)?.normalizedFilePath
                == loopflow.normalizedFilePath
        )
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
