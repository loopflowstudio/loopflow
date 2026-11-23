#if os(macOS)
import Foundation
import Loopflow
import Observation

/// The change list and file sides belong to exactly one comparison.
@MainActor @Observable
final class ComparisonReading {
    let comparison: ChangeComparison
    private(set) var changes: TaskChangesSnapshot?
    private(set) var counts: [String: Int] = [:]
    private(set) var error: String?
    private(set) var loading = false
    @ObservationIgnored private var pending: Task<Void, Never>?

    init(comparison: ChangeComparison) { self.comparison = comparison }

    func refresh(issue: String, query: RegistryQuery, afterWrite: Bool = false) async {
        if let pending {
            await pending.value
            if afterWrite { await refresh(issue: issue, query: query) }
            return
        }
        loading = true
        let read = Task {
            defer { loading = false; pending = nil }
            do {
                let next = try await query.taskChanges(issue: issue, base: comparison.base, head: comparison.head)
                changes = next
                counts = Dictionary(next.files.map { ($0.path, $0.changes) }, uniquingKeysWith: { first, _ in first })
                error = nil
            } catch {
                changes = nil
                counts = [:]
                self.error = error.localizedDescription
            }
        }
        pending = read
        await read.value
    }

    var openedGaps: [String: DiffReviewState.OpenedGaps] = [:]

    private(set) var fileSides: [String: DiffReviewState.FileSides] = [:]
    private(set) var sideErrors: [String: String] = [:]
    @ObservationIgnored private var sideReads: [String: Task<DiffReviewState.FileSides, any Error>] = [:]

    private var sideCommits: DiffReviewState.FileSides.Commits? {
        changes.map { .init(base: $0.baseCommit, head: comparison.head == nil ? nil : $0.headCommit) }
    }

    /// The file's two sides once read for this comparison; asking starts the read.
    func sides(of file: ReviewFile, query: RegistryQuery, issue: String) -> DiffReviewState.FileSides? {
        guard let commits = sideCommits else { return nil }
        if let known = fileSides[file.path], known.commits == commits { return known }
        if sideReads[file.path] == nil, sideErrors[file.path] == nil {
            Task { _ = try? await readSides(file, query: query, issue: issue) }
        }
        return nil
    }

    /// Read again after a failure.
    func retrySides(_ file: ReviewFile, query: RegistryQuery, issue: String) async {
        sideErrors[file.path] = nil
        _ = try? await readSides(file, query: query, issue: issue)
    }

    func readSides(_ file: ReviewFile, query: RegistryQuery, issue: String) async throws -> DiffReviewState.FileSides {
        guard let commits = sideCommits else { throw RegistryQueryError("The comparison has not been read yet.") }
        if let known = fileSides[file.path], known.commits == commits { return known }
        let read = sideReads[file.path] ?? Task { [query, issue] in
            func side(_ path: String?, at commit: String?) async throws -> TaskFileSnapshot? {
                guard let path else { return nil }
                return try await query.taskFile(issue: issue, path: path, at: commit)
            }
            async let old = side(file.status == .added ? nil : file.oldPath ?? file.path, at: commits.base)
            async let new = side(file.status == .deleted ? nil : file.path, at: commits.head)
            return DiffReviewState.FileSides(commits: commits, old: try await old, new: try await new)
        }
        sideReads[file.path] = read
        defer { if sideReads[file.path] == read { sideReads[file.path] = nil } }
        do {
            let result = try await read.value
            if !read.isCancelled, sideCommits == commits {
                fileSides[file.path] = result
                sideErrors[file.path] = nil
            }
            return result
        } catch {
            if !read.isCancelled, sideCommits == commits { sideErrors[file.path] = error.localizedDescription }
            throw error
        }
    }

    /// Forget a file's sides, or every file's, once what they were read for has changed.
    func dropSides(_ path: String? = nil) {
        if let path {
            fileSides[path] = nil
            sideErrors[path] = nil
            sideReads.removeValue(forKey: path)?.cancel()
        } else {
            fileSides = [:]
            sideErrors = [:]
            for read in sideReads.values { read.cancel() }
            sideReads = [:]
        }
    }

}
#endif
