import Foundation

/// The repo a wave's on-disk state lives under.
///
/// A running wave publishes `.lf/wave/<name>/.wave-endpoint` and its journal to the
/// ORIGIN repo — the main checkout — never to a worktree. Mirrors Rust
/// `wave_origin` in `rust/loopflow/src/work/wave/context.rs` (keep the two in
/// skill): a linked worktree's root resolves to the checkout that holds its
/// shared Git directory; the main checkout, a non-git path and a plain
/// subdirectory of a repo resolve unchanged.
///
/// Resolution is memoized per path: the endpoint poll asks every second, and a
/// checkout doesn't stop being a worktree mid-session.
public enum WaveOrigin {
    private static let lock = NSLock()
    nonisolated(unsafe) private static var cache: [String: String] = [:]
    nonisolated(unsafe) private static var changes = 0

    /// Counts recorded origins. A caller that derives values from `cached` can
    /// keep them until this moves.
    public static var revision: Int {
        lock.lock()
        defer { lock.unlock() }
        return changes
    }

    public static func resolve(_ repoPath: String) -> String {
        let key = cacheKey(repoPath)
        lock.lock()
        if let cached = cache[key] {
            lock.unlock()
            return cached
        }
        lock.unlock()
        let origin = resolveUncached(repoPath)
        lock.lock()
        cache[key] = origin
        changes += 1
        lock.unlock()
        return origin
    }

    /// Return a previously resolved origin without performing I/O.
    ///
    /// Render paths use this after their query boundary has called `resolve` on
    /// a detached task. A cache miss stays at the supplied path; SwiftUI must
    /// never run `git` while evaluating its attribute graph.
    public static func cached(_ repoPath: String) -> String {
        let key = cacheKey(repoPath)
        lock.lock()
        defer { lock.unlock() }
        return cache[key] ?? repoPath
    }

    /// Record a path already known to be its own origin, so resolving it runs no `git`.
    public static func remember(origin: String) {
        lock.lock()
        defer { lock.unlock() }
        cache[cacheKey(origin)] = origin
        changes += 1
    }

    private static func cacheKey(_ repoPath: String) -> String {
        let normalized = URL(fileURLWithPath: repoPath).standardizedFileURL.path
        guard normalized != "/" else { return normalized }
        return normalized.trimmingCharacters(in: CharacterSet(charactersIn: "/"))
    }

    /// A linked worktree's `.git` file names its Git directory, whose `commondir`
    /// names the shared one; the main checkout holds that as `.git`. Reading the
    /// two files costs no process: a window resolves every repository its first
    /// readings name before it shows them.
    private static func resolveUncached(_ repoPath: String) -> String {
        let root = URL(fileURLWithPath: repoPath)
        guard let marker = try? String(contentsOf: root.appendingPathComponent(".git"), encoding: .utf8),
              marker.hasPrefix("gitdir:") else { return repoPath }
        let gitDir = URL(fileURLWithPath: marker.dropFirst("gitdir:".count)
            .trimmingCharacters(in: .whitespacesAndNewlines), relativeTo: root)
        guard let shared = try? String(contentsOf: gitDir.appendingPathComponent("commondir"), encoding: .utf8)
        else { return repoPath }
        let common = URL(fileURLWithPath: shared.trimmingCharacters(in: .whitespacesAndNewlines), relativeTo: gitDir)
            .standardizedFileURL
        guard common.lastPathComponent == ".git" else { return repoPath }
        return common.deletingLastPathComponent().path
    }
}
