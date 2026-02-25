import Foundation
import Testing
@testable import Concerto

@Suite("Repo Scanner")
struct RepoScannerTests {
    @Test("scanMainWorktrees returns one-level git repos and skips linked worktrees")
    func scanFiltersRepos() throws {
        let root = try makeTempDirectory(prefix: "repo-scanner")

        let mainRepo = root.appendingPathComponent("main-repo", isDirectory: true)
        try FileManager.default.createDirectory(at: mainRepo, withIntermediateDirectories: true)
        try FileManager.default.createDirectory(
            at: mainRepo.appendingPathComponent(".git", isDirectory: true),
            withIntermediateDirectories: true
        )

        let linkedWorktree = root.appendingPathComponent("linked-worktree", isDirectory: true)
        try FileManager.default.createDirectory(at: linkedWorktree, withIntermediateDirectories: true)
        let linkedGitFile = linkedWorktree.appendingPathComponent(".git")
        try "gitdir: /tmp/demo/.git/worktrees/linked-worktree\n"
            .write(to: linkedGitFile, atomically: true, encoding: .utf8)

        let nonRepo = root.appendingPathComponent("docs", isDirectory: true)
        try FileManager.default.createDirectory(at: nonRepo, withIntermediateDirectories: true)

        let scanner = RepoScanner()
        let found = scanner.scanMainWorktrees(in: root)
        let foundPaths = Set(found.map { $0.path(percentEncoded: false) })

        #expect(foundPaths.contains(mainRepo.standardizedFileURL.path(percentEncoded: false)))
        #expect(!foundPaths.contains(linkedWorktree.standardizedFileURL.path(percentEncoded: false)))
        #expect(!foundPaths.contains(nonRepo.standardizedFileURL.path(percentEncoded: false)))
    }

    private func makeTempDirectory(prefix: String) throws -> URL {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("\(prefix)-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        return directory
    }
}
