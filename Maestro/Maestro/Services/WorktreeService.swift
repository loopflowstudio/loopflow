// Service for interacting with `wt` CLI for worktree operations.

import Foundation

struct WorktreeService {
    enum WorktreeError: LocalizedError {
        case commandFailed(String)
        case parseError(String)
        case notFound
        case wtNotInstalled

        var errorDescription: String? {
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

    func list(in repoURL: URL) async throws -> [Worktree] {
        let output = try await run(["-C", repoURL.path(), "list", "--format", "json", "--full"], in: repoURL)

        guard let data = output.data(using: .utf8) else {
            return []
        }

        let decoder = JSONDecoder()
        let items = try decoder.decode([WorktreeJSON].self, from: data)

        // Filter to actual worktrees (not just branches)
        var worktrees: [Worktree] = []
        for json in items where json.kind != "branch" {
            let hasWorkspace = checkForCodeWorkspace(at: URL(fileURLWithPath: json.path))
            let sessions = (try? await sessionService.history(for: json.path, limit: 10)) ?? []
            worktrees.append(Worktree(from: json, hasCodeWorkspace: hasWorkspace, recentTasks: sessions))
        }
        return worktrees
    }

    func create(name: String, in repoURL: URL, baseBranch: String? = nil) async throws {
        var args = ["-C", repoURL.path(), "switch", "--create", name]
        if let base = baseBranch {
            args.append(contentsOf: ["--base", base])
        }

        _ = try await run(args, in: repoURL)
    }

    func remove(name: String, in repoURL: URL) async throws {
        _ = try await run(["-C", repoURL.path(), "remove", name], in: repoURL)
    }

    func createPR(in worktreePath: URL) async throws {
        _ = try await runLfops(["pr"], in: worktreePath)
    }

    func landPR(in worktreePath: URL) async throws {
        _ = try await runLfops(["land"], in: worktreePath)
    }

func createDraftPR(branch: String, in worktreePath: URL) async throws {
        guard let ghURL = findCommand("gh") else {
            throw WorktreeError.commandFailed("gh CLI not found. Install GitHub CLI.")
        }
        _ = try await runProcess(ghURL, ["pr", "create", "--draft", "--fill"], in: worktreePath)
    }

    func markPRReady(in worktreePath: URL) async throws {
        guard let ghURL = findCommand("gh") else {
            throw WorktreeError.commandFailed("gh CLI not found. Install GitHub CLI.")
        }
        _ = try await runProcess(ghURL, ["pr", "ready"], in: worktreePath)
    }

    func branchIsPushed(_ branch: String, in repoURL: URL) async -> Bool {
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

    func sync(in repoURL: URL) async throws {
        _ = try await runLfops(["sync"], in: repoURL)
    }

    func prune(in repoURL: URL, dryRun: Bool = false) async throws -> [String] {
        var args = ["prune", "--force"]
        if dryRun {
            args.append("--dry-run")
        }
        let output = try await runLfops(args, in: repoURL)

        // Parse output to get branch names
        // Output format: "Would remove:" or "Removed:" followed by "  branch-name" lines
        var branches: [String] = []
        for line in output.components(separatedBy: "\n") {
            let trimmed = line.trimmingCharacters(in: .whitespaces)
            if !trimmed.isEmpty && !trimmed.contains(":") && !trimmed.starts(with: "Syncing") && !trimmed.starts(with: "No merged") {
                branches.append(trimmed)
            }
        }
        return branches
    }

    func getDiff(_ spec: String, in repoURL: URL) async throws -> String {
        try await runProcess(URL(fileURLWithPath: "/usr/bin/git"), ["diff", spec], in: repoURL)
    }

    func getDiffStats(_ spec: String, in repoURL: URL) async throws -> [FileDiffStat] {
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

    func getCommits(for worktree: Worktree, since: String = "main") async throws -> [CommitInfo] {
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

    func getGitHubCompareURL(branch: String, in repoURL: URL, base: String = "main") async throws -> URL? {
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
}
