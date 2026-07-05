import Foundation

/// The repo a wave's on-disk state lives under.
///
/// A running wave publishes `wave/<name>/.wave-endpoint` and its journal to the
/// ORIGIN repo — the main checkout — never to a worktree. Mirrors Rust
/// `wave_origin` in `rust/loopflow/src/engine/wave_context.rs` (keep the two in
/// step): a path that is itself a working-tree root resolves to the parent of
/// its shared git dir (`git rev-parse --git-common-dir`); the main checkout
/// resolves to itself; a non-git path, a plain subdirectory of a repo, or any
/// git failure resolves unchanged.
///
/// Resolution is memoized per path: the endpoint poll asks every second, and a
/// checkout doesn't stop being a worktree mid-session — resolve once, never
/// exec git on the poll loop.
public enum WaveOrigin {
    private static let lock = NSLock()
    nonisolated(unsafe) private static var cache: [String: String] = [:]

    public static func resolve(_ repoPath: String) -> String {
        lock.lock()
        if let cached = cache[repoPath] {
            lock.unlock()
            return cached
        }
        lock.unlock()
        let origin = resolveUncached(repoPath)
        lock.lock()
        cache[repoPath] = origin
        lock.unlock()
        return origin
    }

    #if os(macOS)
    private static func resolveUncached(_ repoPath: String) -> String {
        // Only a working-tree root resolves. A plain subdirectory inside a
        // repo must not walk up into the enclosing checkout (mirrors Rust
        // `is_worktree_root`).
        guard let toplevel = git(["rev-parse", "--show-toplevel"], at: repoPath),
              canonical(toplevel) == canonical(repoPath)
        else {
            return repoPath
        }
        // The shared git dir is `<origin>/.git` for the main checkout and for
        // every worktree alike; its parent is the origin repo (mirrors Rust
        // `main_repo_root`). The main checkout therefore resolves to itself.
        guard let commonDir = git(
            ["rev-parse", "--path-format=absolute", "--git-common-dir"], at: repoPath
        ) else {
            return repoPath
        }
        return URL(fileURLWithPath: commonDir).deletingLastPathComponent().path
    }

    private static func canonical(_ path: String) -> String {
        URL(fileURLWithPath: path).resolvingSymlinksInPath().standardizedFileURL.path
    }

    private static func git(_ args: [String], at path: String) -> String? {
        let process = Process()
        let stdout = Pipe()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/env")
        process.arguments = ["git", "-C", path] + args
        process.standardOutput = stdout
        process.standardError = Pipe()
        do {
            try process.run()
        } catch {
            return nil
        }
        let data = stdout.fileHandleForReading.readDataToEndOfFile()
        process.waitUntilExit()
        guard process.terminationStatus == 0 else { return nil }
        let output = String(decoding: data, as: UTF8.self)
            .trimmingCharacters(in: .whitespacesAndNewlines)
        return output.isEmpty ? nil : output
    }
    #else
    // No local repos (or Process) off macOS; a path is its own origin.
    private static func resolveUncached(_ repoPath: String) -> String { repoPath }
    #endif
}
