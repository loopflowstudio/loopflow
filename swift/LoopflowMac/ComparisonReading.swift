#if os(macOS)
import Foundation
import Loopflow
import Observation

/// The change list and file sides belong to exactly one comparison.
@MainActor @Observable
final class ComparisonReading {
    let issue: String
    let query: RegistryQuery
    let comparison: ChangeComparison
    private(set) var changes: TaskChangesSnapshot? {
        didSet {
            if changes?.baseCommit != oldValue?.baseCommit || changes?.headCommit != oldValue?.headCommit {
                openedGaps = [:]
                dropSides()
            }
        }
    }
    private(set) var filesByPath: [String: TaskChangedFile] = [:]
    private(set) var error: String?
    /// The Task has no checkout on its Machine yet.
    private(set) var notStarted = false
    var loading: Bool { pending != nil }
    private var pending: Task<Void, Never>?

    init(issue: String, query: RegistryQuery, comparison: ChangeComparison) {
        self.issue = issue
        self.query = query
        self.comparison = comparison
    }

    func refresh(afterWrite: Bool = false) async {
        if let pending {
            await pending.value
            if afterWrite { await refresh() }
            return
        }
        let read = Task {
            defer { pending = nil }
            do {
                let next = try await query.taskChanges(issue: issue, base: comparison.base, head: comparison.head)
                changes = next
                filesByPath = Dictionary(next.files.map { ($0.path, $0) }, uniquingKeysWith: { first, _ in first })
                error = nil
                notStarted = false
            } catch {
                changes = nil
                filesByPath = [:]
                notStarted = (error as? ServerError)?.notStarted == true
                self.error = error.localizedDescription
            }
        }
        pending = read
        await read.value
    }

    /// Both versions of a changed file, read once for the comparison's
    /// commits: the old at its base (under its old name), the new at its head,
    /// or in the working tree when the comparison has none. Syntax colors,
    /// skipped lines and images all read them.
    struct FileSides: Equatable, Sendable {
        /// Nil for an added file.
        let old: TaskFileSnapshot?
        /// Nil for a deleted file.
        let new: TaskFileSnapshot?
    }

    /// What opening a file's skipped lines has given so far.
    struct OpenedGaps {
        /// The unchanged lines you opened between hunks, by hunk header.
        var lines: [String: [ReviewLine]] = [:]
        /// The Machine's message from the last open that failed.
        var failure: String?
    }

    var openedGaps: [String: OpenedGaps] = [:]

    private var fileSides: [String: FileSides] = [:]
    private(set) var sideErrors: [String: String] = [:]
    @ObservationIgnored private var sideReads: [String: Task<FileSides, any Error>] = [:]

    /// The file's two sides once read for this comparison; asking starts the read.
    func sides(of file: ReviewFile) -> FileSides? {
        guard changes != nil else { return nil }
        if let known = fileSides[file.path] { return known }
        if sideReads[file.path] == nil, sideErrors[file.path] == nil {
            Task { _ = try? await readSides(file) }
        }
        return nil
    }

    /// Read again after a failure.
    func retrySides(_ file: ReviewFile) async {
        sideErrors[file.path] = nil
        _ = try? await readSides(file)
    }

    func readSides(_ file: ReviewFile) async throws -> FileSides {
        guard let changes else { throw RegistryQueryError("The comparison has not been read yet.") }
        if let known = fileSides[file.path] { return known }
        if let read = sideReads[file.path] { return try await read.value }
        let head = comparison.head == nil ? nil : changes.headCommit
        let read = Task { [query, issue] in
            // dropSides removes cancelled reads. A late reply must leave its replacement alone.
            defer { if !Task.isCancelled { sideReads[file.path] = nil } }
            func side(_ path: String?, at commit: String?) async throws -> TaskFileSnapshot? {
                guard let path else { return nil }
                return try await query.taskFile(issue: issue, path: path, at: commit)
            }
            do {
                async let old = side(file.status == .added ? nil : file.oldPath ?? file.path, at: changes.baseCommit)
                async let new = side(file.status == .deleted ? nil : file.path, at: head)
                let sides = try await FileSides(old: old, new: new)
                try Task.checkCancellation()
                fileSides[file.path] = sides
                sideErrors[file.path] = nil
                return sides
            } catch {
                try Task.checkCancellation()
                sideErrors[file.path] = error.localizedDescription
                throw error
            }
        }
        sideReads[file.path] = read
        return try await read.value
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
