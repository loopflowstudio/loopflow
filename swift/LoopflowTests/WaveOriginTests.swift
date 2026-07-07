// Origin resolution for wave state: a worktree resolves to its main checkout,
// everything else to itself. Exercised against real git repos in a temp dir —
// the helper's entire job is interpreting git output, so mocking git would
// prove nothing. Mirrors Rust `wave_origin` (rust/loopflow/src/engine/
// wave_context.rs); if these semantics move, move them in both places.

#if os(macOS)
import Foundation
import Testing
@testable import LoopflowCore

@Suite("Wave origin resolution")
struct WaveOriginTests {
    /// Canonicalize before comparing: macOS temp dirs live behind the
    /// /var -> /private/var symlink, and git reports the /private side.
    private func canonical(_ path: String) -> String {
        URL(fileURLWithPath: path).resolvingSymlinksInPath().standardizedFileURL.path
    }

    private func git(_ args: [String], at dir: URL) throws {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/env")
        process.arguments = ["git", "-C", dir.path] + args
        process.standardOutput = Pipe()
        process.standardError = Pipe()
        try process.run()
        process.waitUntilExit()
        try #require(process.terminationStatus == 0, "git \(args.joined(separator: " "))")
    }

    private func withTempDir(_ body: (URL) throws -> Void) throws {
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("wave-origin-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root) }
        try body(root)
    }

    @Test("a worktree resolves to its origin repo; the origin to itself")
    func worktreeResolvesToOrigin() throws {
        try withTempDir { root in
            let origin = root.appendingPathComponent("repo", isDirectory: true)
            let worktree = root.appendingPathComponent("repo.wt", isDirectory: true)
            try FileManager.default.createDirectory(at: origin, withIntermediateDirectories: true)
            try git(["init", "-q"], at: origin)
            try git(
                ["-c", "user.email=t@t", "-c", "user.name=t",
                 "commit", "--allow-empty", "-q", "-m", "init"],
                at: origin
            )
            try git(["worktree", "add", "-q", worktree.path], at: origin)

            #expect(canonical(WaveOrigin.resolve(worktree.path)) == canonical(origin.path))
            #expect(canonical(WaveOrigin.resolve(origin.path)) == canonical(origin.path))
        }
    }

    @Test("a plain subdirectory of a repo is its own origin — no walking up")
    func subdirectoryDoesNotWalkUp() throws {
        try withTempDir { root in
            let repo = root.appendingPathComponent("repo", isDirectory: true)
            let subdir = repo.appendingPathComponent("wave", isDirectory: true)
            try FileManager.default.createDirectory(at: subdir, withIntermediateDirectories: true)
            try git(["init", "-q"], at: repo)

            #expect(WaveOrigin.resolve(subdir.path) == subdir.path)
        }
    }

    @Test("non-git paths resolve unchanged")
    func nonGitPathsUnchanged() throws {
        try withTempDir { root in
            let plain = root.appendingPathComponent("plain", isDirectory: true)
            try FileManager.default.createDirectory(at: plain, withIntermediateDirectories: true)

            #expect(WaveOrigin.resolve(plain.path) == plain.path)
            let missing = root.appendingPathComponent("does-not-exist").path
            #expect(WaveOrigin.resolve(missing) == missing)
        }
    }
}
#endif
