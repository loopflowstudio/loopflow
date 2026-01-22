// Service for interacting with `wt` CLI for worktree operations.

import Foundation

public struct WorktreeService: @unchecked Sendable {
    public init() {}

    public enum WorktreeError: LocalizedError {
        case commandFailed(String)
        case parseError(String)
        case notFound
        case wtNotInstalled

        public var errorDescription: String? {
            switch self {
            case .commandFailed(let msg):
                if msg.contains("already exists") {
                    return "A branch with this name already exists. Try a different name."
                }
                return msg.trimmingCharacters(in: .whitespacesAndNewlines)
            case .parseError(let msg): return "Couldn't read worktree data: \(msg)"
            case .notFound: return "This worktree no longer exists. Try refreshing."
            case .wtNotInstalled: return "Worktrunk not installed. Click retry to install it."
            }
        }
    }

    private func findCommand(_ name: String) -> URL? {
        let process = Process()
        let pipe = Pipe()
        process.executableURL = URL(fileURLWithPath: "/bin/zsh")
        process.arguments = ["-l", "-c", "which \(name)"]
        process.standardOutput = pipe
        process.standardError = FileHandle.nullDevice

        do {
            try process.run()
            process.waitUntilExit()
            if process.terminationStatus == 0 {
                let data = pipe.fileHandleForReading.readDataToEndOfFile()
                if let path = String(data: data, encoding: .utf8)?.trimmingCharacters(in: .whitespacesAndNewlines),
                   !path.isEmpty {
                    return URL(fileURLWithPath: path)
                }
            }
        } catch {
            // Fall through to return nil
        }
        return nil
    }

    private let sessionService = SessionService()
    @MainActor private static var lfopsCompatible: Bool?

    private func runProcessWithStatus(
        _ executable: URL,
        _ args: [String],
        in directory: URL,
        fallbackToWhich: Bool = false
    ) async throws -> (String, Int32) {
        let execURL: URL
        if fallbackToWhich, let resolved = findCommand(executable.lastPathComponent) {
            execURL = resolved
        } else {
            execURL = executable
        }

        return try await withCheckedThrowingContinuation { continuation in
            let process = Process()
            let pipe = Pipe()

            process.executableURL = execURL
            process.arguments = args
            process.currentDirectoryURL = directory
            process.standardOutput = pipe
            process.standardError = pipe

            do {
                try process.run()
                process.waitUntilExit()

                let data = pipe.fileHandleForReading.readDataToEndOfFile()
                let output = String(data: data, encoding: .utf8) ?? ""
                continuation.resume(returning: (output, process.terminationStatus))
            } catch {
                continuation.resume(throwing: error)
            }
        }
    }

    /// List worktrees. Use `full: false` for fast initial load (skips diff stats, CI, session history).
    public func list(in repoURL: URL, full: Bool = true) async throws -> [Worktree] {
        let startTime = CFAbsoluteTimeGetCurrent()
        var args = ["wt", "list", "--format", "json"]
        if full { args.append("--full") }

        let lfopsCompatible = await getLfopsCompatible()
        if lfopsCompatible != false, let lfopsURL = findCommand("lfops") {
            do {
                let t0 = CFAbsoluteTimeGetCurrent()
                let (output, status) = try await runProcessWithStatus(lfopsURL, args, in: repoURL)
                let cmdTime = Int((CFAbsoluteTimeGetCurrent() - t0) * 1000)
                LoggingService.append("worktrees.list command=lfops full=\(full) status=\(status) bytes=\(output.utf8.count) cmd_ms=\(cmdTime)")
                if status == 0 {
                    let t1 = CFAbsoluteTimeGetCurrent()
                    if let worktrees = try? await decodeWorktrees(from: output, source: "lfops", loadSessions: full) {
                        let decodeTime = Int((CFAbsoluteTimeGetCurrent() - t1) * 1000)
                        let totalTime = Int((CFAbsoluteTimeGetCurrent() - startTime) * 1000)
                        LoggingService.append("worktrees.list decode_ms=\(decodeTime) total_ms=\(totalTime)")
                        await setLfopsCompatible(true)
                        return worktrees
                    }
                }
                if output.contains("No such command 'wt'") {
                    await setLfopsCompatible(false)
                    LoggingService.append("worktrees.list command=lfops status=disabled reason=no-wt-command")
                }
            } catch {
                LoggingService.append("worktrees.list command=lfops error=\(error.localizedDescription)")
                if error.localizedDescription.contains("No such command 'wt'") {
                    await setLfopsCompatible(false)
                }
            }
        }

        // Fallback to wt directly
        var wtArgs = ["list", "--format", "json"]
        if full { wtArgs.append("--full") }

        let t0 = CFAbsoluteTimeGetCurrent()
        let (output, status) = try await runProcessWithStatus(
            URL(fileURLWithPath: "/opt/homebrew/bin/wt"),
            wtArgs,
            in: repoURL,
            fallbackToWhich: true
        )
        let cmdTime = Int((CFAbsoluteTimeGetCurrent() - t0) * 1000)
        LoggingService.append("worktrees.list command=wt full=\(full) status=\(status) bytes=\(output.utf8.count) cmd_ms=\(cmdTime)")
        if status != 0 {
            throw WorktreeError.commandFailed(output)
        }
        let t1 = CFAbsoluteTimeGetCurrent()
        let result = try await decodeWorktrees(from: output, source: "wt", loadSessions: full)
        let decodeTime = Int((CFAbsoluteTimeGetCurrent() - t1) * 1000)
        let totalTime = Int((CFAbsoluteTimeGetCurrent() - startTime) * 1000)
        LoggingService.append("worktrees.list decode_ms=\(decodeTime) total_ms=\(totalTime)")
        return result
    }

    private func decodeWorktrees(from output: String, source: String, loadSessions: Bool) async throws -> [Worktree] {
        guard let data = output.data(using: .utf8) else {
            return []
        }

        let decoder = JSONDecoder()
        let items: [WorktreeJSON]
        do {
            items = try decoder.decode([WorktreeJSON].self, from: data)
        } catch {
            let snippet = output.trimmingCharacters(in: .whitespacesAndNewlines).prefix(200)
            LoggingService.append("worktrees.parse source=\(source) error=\(error.localizedDescription) preview=\(snippet)")
            throw error
        }

        // Filter to actual worktrees (not just branches)
        var worktrees: [Worktree] = []
        for json in items where json.kind != "branch" {
            let hasWorkspace = checkForCodeWorkspace(at: URL(fileURLWithPath: json.path))
            let sessions = loadSessions ? ((try? await sessionService.history(for: json.path, limit: 10)) ?? []) : []
            worktrees.append(Worktree(from: json, hasCodeWorkspace: hasWorkspace, recentSteps: sessions))
        }
        LoggingService.append("worktrees.parse source=\(source) count=\(worktrees.count)")
        return worktrees
    }

    private func getLfopsCompatible() async -> Bool? {
        await MainActor.run { WorktreeService.lfopsCompatible }
    }

    private func setLfopsCompatible(_ value: Bool) async {
        await MainActor.run { WorktreeService.lfopsCompatible = value }
    }

    public func create(name: String, in repoURL: URL, baseBranch: String? = nil) async throws {
        // Use lfops wt create for schema-based branch naming
        var args = ["wt", "create", name]
        if let base = baseBranch {
            args.append(contentsOf: ["--base", base])
        }

        _ = try await runLfops(args, in: repoURL)
    }

    public func remove(name: String, in repoURL: URL) async throws {
        _ = try await run(["-C", repoURL.path(), "remove", name], in: repoURL)
    }

    public func createPR(in worktreePath: URL) async throws {
        _ = try await runLfops(["pr"], in: worktreePath)
    }

    public func landPR(in worktreePath: URL) async throws {
        _ = try await runLfops(["land"], in: worktreePath)
    }

    public func landBranch(in worktreePath: URL) async throws {
        _ = try await runLfops(["land", "--create-pr"], in: worktreePath)
    }

    public func createDraftPR(branch: String, in worktreePath: URL) async throws {
        guard let ghURL = findCommand("gh") else {
            throw WorktreeError.commandFailed("gh CLI not found. Install GitHub CLI.")
        }
        _ = try await runProcess(ghURL, ["pr", "create", "--draft", "--fill"], in: worktreePath)
    }

    public func markPRReady(in worktreePath: URL) async throws {
        guard let ghURL = findCommand("gh") else {
            throw WorktreeError.commandFailed("gh CLI not found. Install GitHub CLI.")
        }
        _ = try await runProcess(ghURL, ["pr", "ready"], in: worktreePath)
    }

    public func getExistingPRURL(in worktreePath: URL) async -> URL? {
        guard let ghURL = findCommand("gh") else {
            return nil
        }
        do {
            let output = try await runProcess(ghURL, ["pr", "view", "--json", "url", "-q", ".url"], in: worktreePath)
            let trimmed = output.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !trimmed.isEmpty else { return nil }
            return URL(string: trimmed)
        } catch {
            return nil
        }
    }

    public func getCIStatus(for branch: String, in repoURL: URL) async -> CIStatus {
        guard let ghURL = findCommand("gh") else {
            return .unknown
        }
        do {
            let (output, status) = try await runProcessWithStatus(
                ghURL,
                ["pr", "checks", branch, "--json", "state", "-q", ".[].state"],
                in: repoURL
            )
            if status != 0 {
                return .unknown
            }
            let states = output.trimmingCharacters(in: .whitespacesAndNewlines)
                .split(separator: "\n")
                .map { String($0) }

            if states.isEmpty {
                return .unknown
            }
            if states.contains("FAILURE") || states.contains("ERROR") {
                return .failing
            }
            if states.contains("PENDING") || states.contains("QUEUED") || states.contains("IN_PROGRESS") {
                return .pending
            }
            if states.allSatisfy({ $0 == "SUCCESS" || $0 == "SKIPPED" || $0 == "NEUTRAL" }) {
                return .passing
            }
            return .unknown
        } catch {
            return .unknown
        }
    }

    public func getCIStatusForAll(_ worktrees: [Worktree], in repoURL: URL) async -> [String: CIStatus] {
        var results: [String: CIStatus] = [:]
        for worktree in worktrees where worktree.prNumber != nil {
            results[worktree.branch] = await getCIStatus(for: worktree.branch, in: repoURL)
        }
        return results
    }

    public func branchIsPushed(_ branch: String, in repoURL: URL) async -> Bool {
        do {
            let output = try await runProcess(
                URL(fileURLWithPath: "/usr/bin/git"),
                ["ls-remote", "--heads", "origin", branch],
                in: repoURL
            )
            return !output.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        } catch {
            return false
        }
    }

    public func hasDiffAgainstBase(_ worktree: Worktree) async -> Bool {
        let base = worktree.baseBranch ?? "main"
        let worktreeURL = URL(fileURLWithPath: worktree.path)
        do {
            let output = try await runProcess(
                URL(fileURLWithPath: "/usr/bin/git"),
                ["diff", "--name-only", "\(base)...HEAD"],
                in: worktreeURL
            )
            return !output.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        } catch {
            return true
        }
    }

    public func sync(in repoURL: URL) async throws {
        _ = try await runLfops(["sync"], in: repoURL)
    }

    public func prune(in repoURL: URL, dryRun: Bool = false) async throws -> [String] {
        var args = ["prune", "--force"]
        if dryRun {
            args.append("--dry-run")
        }
        let output = try await runLfops(args, in: repoURL)

        return parsePrunableBranches(from: output)
    }

    public func getDiff(_ spec: String, in repoURL: URL) async throws -> String {
        try await runProcess(URL(fileURLWithPath: "/usr/bin/git"), ["diff", spec], in: repoURL)
    }

    public func getDiffStats(_ spec: String, in repoURL: URL) async throws -> [FileDiffStat] {
        // Use --numstat for machine-readable output: additions, deletions, filename
        let output = try await runProcess(
            URL(fileURLWithPath: "/usr/bin/git"),
            ["diff", spec, "--numstat"],
            in: repoURL
        )

        return output
            .split(separator: "\n")
            .compactMap { line -> FileDiffStat? in
                let parts = line.split(separator: "\t")
                guard parts.count >= 3 else { return nil }

                // Binary files show "-" for additions/deletions
                let additions = Int(parts[0]) ?? 0
                let deletions = Int(parts[1]) ?? 0
                let path = String(parts[2])

                return FileDiffStat(
                    id: path,
                    path: path,
                    additions: additions,
                    deletions: deletions
                )
            }
            .sorted { $0.totalChanges > $1.totalChanges }
    }

    public func getCommits(for worktree: Worktree, since: String = "main") async throws -> [CommitInfo] {
        let worktreeURL = URL(fileURLWithPath: worktree.path)
        // Message last so pipes in commit messages don't break parsing
        let format = "%H|%h|%an|%aI|%s"
        let range = "\(since)..HEAD"

        let output = try await runProcess(
            URL(fileURLWithPath: "/usr/bin/git"),
            ["log", range, "--format=\(format)"],
            in: worktreeURL
        )

        let dateFormatter = ISO8601DateFormatter()
        dateFormatter.formatOptions = [.withInternetDateTime]

        return output
            .split(separator: "\n")
            .compactMap { line -> CommitInfo? in
                let parts = line.split(separator: "|", maxSplits: 4)
                guard parts.count >= 5 else { return nil }

                let sha = String(parts[0])
                let shortSHA = String(parts[1])
                let author = String(parts[2])
                let dateString = String(parts[3])
                let message = String(parts[4])

                guard let date = dateFormatter.date(from: dateString) else { return nil }

                return CommitInfo(
                    id: sha,
                    shortSHA: shortSHA,
                    message: message,
                    author: author,
                    date: date
                )
            }
    }

    public func detectStaleness(for worktree: Worktree, in repoURL: URL) async -> Staleness {
        // Skip main branch - it's never stale
        if worktree.branch == "main" || worktree.branch == "master" {
            LoggingService.append("""
            worktrees.staleness branch=\(worktree.branch) staleness=Active
            """)
            return .active
        }

        let baseBranch = worktree.baseBranch ?? "main"
        // Only mark as merged if the worktree has commits (aheadMain > 0).
        // A worktree with 0 commits ahead and no diff is just new, not merged.
        if worktree.aheadMain > 0,
           (try? await runProcess(
            URL(fileURLWithPath: "/usr/bin/git"),
            ["diff", "--quiet", "\(baseBranch)...\(worktree.branch)"],
            in: repoURL
        )) != nil {
            LoggingService.append("""
            worktrees.staleness branch=\(worktree.branch) staleness=Merged
            """)
            return .merged
        }

        // Check if branch is merged to main
        if let merged = try? await runGit(["branch", "--merged", "main"], in: repoURL) {
            let mergedBranches = merged.split(separator: "\n").map { $0.trimmingCharacters(in: .whitespaces) }
            if mergedBranches.contains(worktree.branch) {
                LoggingService.append("""
                worktrees.staleness branch=\(worktree.branch) staleness=Merged
                """)
                return .merged
            }
        }

        // Check if remote tracking branch exists
        let remoteRef = "refs/remotes/origin/\(worktree.branch)"
        let refExists = try? await runGit(["show-ref", "--verify", remoteRef], in: repoURL)
        if refExists == nil && worktree.prNumber != nil {
            // Had a PR but remote branch is gone → merged and cleaned up
            LoggingService.append("""
            worktrees.staleness branch=\(worktree.branch) staleness=Remote deleted
            """)
            return .remoteDeleted
        }

        // Check last commit date
        if let lastCommitTime = try? await runGit(["log", "-1", "--format=%ct", worktree.branch], in: repoURL) {
            let trimmed = lastCommitTime.trimmingCharacters(in: .whitespacesAndNewlines)
            if let timestamp = Double(trimmed) {
                let age = Date().timeIntervalSince1970 - timestamp
                let days = Int(age / 86400)
                if days > 14 {
                    let result: Staleness = .inactive(days: days)
                    LoggingService.append("""
                    worktrees.staleness branch=\(worktree.branch) staleness=\(result.displayText)
                    """)
                    return result
                }
            }
        }

        LoggingService.append("""
        worktrees.staleness branch=\(worktree.branch) staleness=Active
        """)
        return .active
    }

    public func detectStalenessForAll(_ worktrees: [Worktree], in repoURL: URL) async -> [String: Staleness] {
        var results: [String: Staleness] = [:]
        for worktree in worktrees {
            results[worktree.branch] = await detectStaleness(for: worktree, in: repoURL)
        }
        return results
    }

    private func runGit(_ args: [String], in directory: URL) async throws -> String {
        try await runProcess(URL(fileURLWithPath: "/usr/bin/git"), args, in: directory)
    }

    public func getPrunableBranches(in repoURL: URL) async -> Set<String> {
        do {
            let output = try await runLfops(["prune", "--dry-run"], in: repoURL)
            return Set(parsePrunableBranches(from: output))
        } catch {
            return []
        }
    }

    public func getGitHubCompareURL(branch: String, in repoURL: URL, base: String = "main") async throws -> URL? {
        let remoteURL = try await runProcess(URL(fileURLWithPath: "/usr/bin/git"), ["remote", "get-url", "origin"], in: repoURL)
            .trimmingCharacters(in: .whitespacesAndNewlines)

        var repoPath: String?
        if remoteURL.hasPrefix("git@github.com:") {
            repoPath = String(remoteURL.dropFirst("git@github.com:".count)).replacingOccurrences(of: ".git", with: "")
        } else if remoteURL.contains("github.com"), let range = remoteURL.range(of: "github.com/") {
            repoPath = String(remoteURL[range.upperBound...]).replacingOccurrences(of: ".git", with: "")
        }

        guard let path = repoPath else { return nil }
        return URL(string: "https://github.com/\(path)/compare/\(base)...\(branch)")
    }

    private func run(_ args: [String], in directory: URL) async throws -> String {
        guard let wtURL = findCommand("wt") else {
            throw WorktreeError.wtNotInstalled
        }
        return try await runProcess(wtURL, args, in: directory)
    }

    private func runLfops(_ args: [String], in directory: URL) async throws -> String {
        guard let lfopsURL = findCommand("lfops") else {
            throw WorktreeError.commandFailed("lfops not found. Install loopflow.")
        }
        return try await runProcess(lfopsURL, args, in: directory)
    }

    private func runProcess(_ executable: URL, _ args: [String], in directory: URL) async throws -> String {
        try await withCheckedThrowingContinuation { continuation in
            let process = Process()
            let pipe = Pipe()

            process.executableURL = executable
            process.arguments = args
            process.currentDirectoryURL = directory
            process.standardOutput = pipe
            process.standardError = pipe

            do {
                try process.run()
                process.waitUntilExit()

                let data = pipe.fileHandleForReading.readDataToEndOfFile()
                let output = String(data: data, encoding: .utf8) ?? ""

                if process.terminationStatus == 0 {
                    continuation.resume(returning: output)
                } else {
                    continuation.resume(throwing: WorktreeError.commandFailed(output))
                }
            } catch {
                continuation.resume(throwing: error)
            }
        }
    }

    private func checkForCodeWorkspace(at url: URL) -> Bool {
        let fm = FileManager.default
        guard let contents = try? fm.contentsOfDirectory(atPath: url.path()) else {
            return false
        }
        return contents.contains { $0.hasSuffix(".code-workspace") }
    }

    private func parsePrunableBranches(from output: String) -> [String] {
        var branches: [String] = []
        for line in output.components(separatedBy: "\n") {
            let trimmed = line.trimmingCharacters(in: .whitespaces)
            if !trimmed.isEmpty && !trimmed.contains(":") && !trimmed.starts(with: "Syncing") && !trimmed.starts(with: "No merged") {
                branches.append(trimmed)
            }
        }
        return branches
    }
}
