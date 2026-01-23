// Service for loading data from lfd.db SQLite database.

import Foundation
import SQLite3

public struct LfdService: @unchecked Sendable {
    public init() {}
    private let dbPath = FileManager.default.homeDirectoryForCurrentUser
        .appendingPathComponent(".lf/lfd.db").path

    // MARK: - Loops

    public func listLoops(repo: URL) async throws -> [Loop] {
        var loops: [Loop] = []

        var db: OpaquePointer?
        guard sqlite3_open(dbPath, &db) == SQLITE_OK else {
            return []
        }
        defer { sqlite3_close(db) }

        let query = """
            SELECT id, flow, area, repo, goals, status, iteration,
                   main_branch, pr_limit, merge_mode, pid, created_at
            FROM loops
            WHERE repo = ?
            ORDER BY created_at DESC
        """

        var stmt: OpaquePointer?
        guard sqlite3_prepare_v2(db, query, -1, &stmt, nil) == SQLITE_OK else {
            return []
        }
        defer { sqlite3_finalize(stmt) }

        sqlite3_bind_text(stmt, 1, repo.path, -1, nil)

        let dateFormatter = ISO8601DateFormatter()
        dateFormatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]

        while sqlite3_step(stmt) == SQLITE_ROW {
            guard let id = columnText(stmt, 0),
                  let flow = columnText(stmt, 1),
                  let repoPath = columnText(stmt, 3),
                  let statusStr = columnText(stmt, 5),
                  let mainBranch = columnText(stmt, 7) else {
                continue
            }

            let area = columnText(stmt, 2) ?? "."
            let goals = decodeGoals(columnText(stmt, 4))
            let iteration = Int(sqlite3_column_int(stmt, 6))
            let prLimit = Int(sqlite3_column_int(stmt, 8))
            let mergeModeStr = columnText(stmt, 9) ?? "pr"
            let pid = sqlite3_column_type(stmt, 10) != SQLITE_NULL
                ? Int(sqlite3_column_int(stmt, 10))
                : nil

            var createdAt = Date()
            if let dateStr = columnText(stmt, 11) {
                createdAt = dateFormatter.date(from: dateStr) ?? Date()
            }

            let status = TriggerStatus(rawValue: statusStr) ?? .idle
            let mergeMode = MergeMode(rawValue: mergeModeStr) ?? .pr

            var loop = Loop(
                id: id,
                flow: flow,
                area: area,
                repo: repoPath,
                goals: goals,
                status: status,
                iteration: iteration,
                mainBranch: mainBranch,
                prLimit: prLimit,
                mergeMode: mergeMode,
                pid: pid,
                createdAt: createdAt
            )

            // Get current run info if running
            if status == .running {
                if let runInfo = getCurrentRun(db: db, parent: "loop:\(id)") {
                    loop.currentRunId = runInfo.id
                    loop.currentStep = runInfo.currentStep
                }
            }

            // Check commits ahead of main
            if let commits = try? await getCommitsAhead(branch: mainBranch, in: URL(fileURLWithPath: repoPath)) {
                loop.commitsAhead = commits
            }

            loops.append(loop)
        }

        return loops
    }

    // MARK: - Runs

    public func listRuns(parent: String? = nil, repo: URL? = nil, limit: Int = 50) async throws -> [Run] {
        var runs: [Run] = []

        var db: OpaquePointer?
        guard sqlite3_open(dbPath, &db) == SQLITE_OK else {
            return []
        }
        defer { sqlite3_close(db) }

        var query = """
            SELECT id, parent, flow, area, repo, goals, status, iteration,
                   worktree, branch, current_step, error, pr_url,
                   started_at, ended_at, created_at
            FROM runs
        """

        var conditions: [String] = []
        if parent != nil {
            conditions.append("parent = ?")
        }
        if repo != nil {
            conditions.append("repo = ?")
        }

        if !conditions.isEmpty {
            query += " WHERE " + conditions.joined(separator: " AND ")
        }
        query += " ORDER BY created_at DESC LIMIT ?"

        var stmt: OpaquePointer?
        guard sqlite3_prepare_v2(db, query, -1, &stmt, nil) == SQLITE_OK else {
            return []
        }
        defer { sqlite3_finalize(stmt) }

        var paramIndex: Int32 = 1
        if let parent {
            sqlite3_bind_text(stmt, paramIndex, parent, -1, nil)
            paramIndex += 1
        }
        if let repo {
            sqlite3_bind_text(stmt, paramIndex, repo.path, -1, nil)
            paramIndex += 1
        }
        sqlite3_bind_int(stmt, paramIndex, Int32(limit))

        let dateFormatter = ISO8601DateFormatter()
        dateFormatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]

        while sqlite3_step(stmt) == SQLITE_ROW {
            guard let id = columnText(stmt, 0),
                  let flow = columnText(stmt, 2),
                  let repoPath = columnText(stmt, 4),
                  let statusStr = columnText(stmt, 6) else {
                continue
            }

            let parent = columnText(stmt, 1)
            let area = columnText(stmt, 3) ?? "."
            let goals = decodeGoals(columnText(stmt, 5))
            let iteration = Int(sqlite3_column_int(stmt, 7))
            let worktree = columnText(stmt, 8)
            let branch = columnText(stmt, 9)
            let currentStep = columnText(stmt, 10)
            let error = columnText(stmt, 11)
            let prUrl = columnText(stmt, 12)

            var startedAt: Date?
            if let dateStr = columnText(stmt, 13) {
                startedAt = dateFormatter.date(from: dateStr)
            }

            var endedAt: Date?
            if let dateStr = columnText(stmt, 14) {
                endedAt = dateFormatter.date(from: dateStr)
            }

            var createdAt = Date()
            if let dateStr = columnText(stmt, 15) {
                createdAt = dateFormatter.date(from: dateStr) ?? Date()
            }

            let status = RunStatus(rawValue: statusStr) ?? .pending

            runs.append(Run(
                id: id,
                parent: parent,
                flow: flow,
                area: area,
                repo: repoPath,
                goals: goals,
                status: status,
                iteration: iteration,
                worktree: worktree,
                branch: branch,
                currentStep: currentStep,
                error: error,
                prUrl: prUrl,
                startedAt: startedAt,
                endedAt: endedAt,
                createdAt: createdAt
            ))
        }

        return runs
    }

    public func getRuns(forLoop loopId: String, limit: Int = 10) async throws -> [Run] {
        return try await listRuns(parent: "loop:\(loopId)", limit: limit)
    }

    // MARK: - Triggers (all types)

    public func listTriggers(repo: URL) async throws -> [Trigger] {
        var triggers: [Trigger] = []

        // Load loops
        let loops = try await listLoops(repo: repo)
        triggers.append(contentsOf: loops.map { .loop($0) })

        // Load subscriptions
        let subscriptions = try await listSubscriptions(repo: repo)
        triggers.append(contentsOf: subscriptions.map { .subscription($0) })

        // Load schedules
        let schedules = try await listSchedules(repo: repo)
        triggers.append(contentsOf: schedules.map { .schedule($0) })

        return triggers
    }

    // MARK: - Subscriptions

    public func listSubscriptions(repo: URL) async throws -> [Subscription] {
        var subscriptions: [Subscription] = []

        var db: OpaquePointer?
        guard sqlite3_open(dbPath, &db) == SQLITE_OK else {
            return []
        }
        defer { sqlite3_close(db) }

        let query = """
            SELECT id, flow, area, repo, goals, pathset, last_main_sha,
                   status, iteration, main_branch, pr_limit, merge_mode, pid, created_at
            FROM subscriptions
            WHERE repo = ?
            ORDER BY created_at DESC
        """

        var stmt: OpaquePointer?
        guard sqlite3_prepare_v2(db, query, -1, &stmt, nil) == SQLITE_OK else {
            return []
        }
        defer { sqlite3_finalize(stmt) }

        sqlite3_bind_text(stmt, 1, repo.path, -1, nil)

        let dateFormatter = ISO8601DateFormatter()
        dateFormatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]

        while sqlite3_step(stmt) == SQLITE_ROW {
            guard let id = columnText(stmt, 0),
                  let flow = columnText(stmt, 1),
                  let repoPath = columnText(stmt, 3),
                  let statusStr = columnText(stmt, 7),
                  let mainBranch = columnText(stmt, 9) else {
                continue
            }

            let area = columnText(stmt, 2) ?? "."
            let goals = decodeGoals(columnText(stmt, 4))
            let pathset = columnText(stmt, 5) ?? ""
            let lastMainSha = columnText(stmt, 6)
            let iteration = Int(sqlite3_column_int(stmt, 8))
            let prLimit = Int(sqlite3_column_int(stmt, 10))
            let mergeModeStr = columnText(stmt, 11) ?? "pr"
            let pid = sqlite3_column_type(stmt, 12) != SQLITE_NULL
                ? Int(sqlite3_column_int(stmt, 12))
                : nil

            var createdAt = Date()
            if let dateStr = columnText(stmt, 13) {
                createdAt = dateFormatter.date(from: dateStr) ?? Date()
            }

            let status = TriggerStatus(rawValue: statusStr) ?? .idle
            let mergeMode = MergeMode(rawValue: mergeModeStr) ?? .pr

            var subscription = Subscription(
                id: id,
                flow: flow,
                area: area,
                repo: repoPath,
                goals: goals,
                pathset: pathset,
                lastMainSha: lastMainSha,
                status: status,
                iteration: iteration,
                mainBranch: mainBranch,
                prLimit: prLimit,
                mergeMode: mergeMode,
                pid: pid,
                createdAt: createdAt
            )

            // Get current run info if running
            if status == .running {
                if let runInfo = getCurrentRun(db: db, parent: "subscription:\(id)") {
                    subscription.currentRunId = runInfo.id
                    subscription.currentStep = runInfo.currentStep
                }
            }

            // Check commits ahead of main
            if let commits = try? await getCommitsAhead(branch: mainBranch, in: URL(fileURLWithPath: repoPath)) {
                subscription.commitsAhead = commits
            }

            subscriptions.append(subscription)
        }

        return subscriptions
    }

    // MARK: - Schedules

    public func listSchedules(repo: URL) async throws -> [Schedule] {
        var schedules: [Schedule] = []

        var db: OpaquePointer?
        guard sqlite3_open(dbPath, &db) == SQLITE_OK else {
            return []
        }
        defer { sqlite3_close(db) }

        let query = """
            SELECT id, flow, area, repo, goals, cron,
                   status, iteration, main_branch, pr_limit, merge_mode, pid, created_at
            FROM schedules
            WHERE repo = ?
            ORDER BY created_at DESC
        """

        var stmt: OpaquePointer?
        guard sqlite3_prepare_v2(db, query, -1, &stmt, nil) == SQLITE_OK else {
            return []
        }
        defer { sqlite3_finalize(stmt) }

        sqlite3_bind_text(stmt, 1, repo.path, -1, nil)

        let dateFormatter = ISO8601DateFormatter()
        dateFormatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]

        while sqlite3_step(stmt) == SQLITE_ROW {
            guard let id = columnText(stmt, 0),
                  let flow = columnText(stmt, 1),
                  let repoPath = columnText(stmt, 3),
                  let statusStr = columnText(stmt, 6),
                  let mainBranch = columnText(stmt, 8) else {
                continue
            }

            let area = columnText(stmt, 2) ?? "."
            let goals = decodeGoals(columnText(stmt, 4))
            let cron = columnText(stmt, 5) ?? ""
            let iteration = Int(sqlite3_column_int(stmt, 7))
            let prLimit = Int(sqlite3_column_int(stmt, 9))
            let mergeModeStr = columnText(stmt, 10) ?? "pr"
            let pid = sqlite3_column_type(stmt, 11) != SQLITE_NULL
                ? Int(sqlite3_column_int(stmt, 11))
                : nil

            var createdAt = Date()
            if let dateStr = columnText(stmt, 12) {
                createdAt = dateFormatter.date(from: dateStr) ?? Date()
            }

            let status = TriggerStatus(rawValue: statusStr) ?? .idle
            let mergeMode = MergeMode(rawValue: mergeModeStr) ?? .pr

            var schedule = Schedule(
                id: id,
                flow: flow,
                area: area,
                repo: repoPath,
                goals: goals,
                cron: cron,
                status: status,
                iteration: iteration,
                mainBranch: mainBranch,
                prLimit: prLimit,
                mergeMode: mergeMode,
                pid: pid,
                createdAt: createdAt
            )

            // Get current run info if running
            if status == .running {
                if let runInfo = getCurrentRun(db: db, parent: "schedule:\(id)") {
                    schedule.currentRunId = runInfo.id
                    schedule.currentStep = runInfo.currentStep
                }
            }

            // Check commits ahead of main
            if let commits = try? await getCommitsAhead(branch: mainBranch, in: URL(fileURLWithPath: repoPath)) {
                schedule.commitsAhead = commits
            }

            schedules.append(schedule)
        }

        return schedules
    }

    // MARK: - Actions

    public func squashLand(trigger: Trigger, repoRoot: URL) async throws {
        guard trigger.commitsAhead > 0 else {
            throw LfdServiceError.nothingToLand
        }

        // Checkout main
        _ = try await runGitCommand(["checkout", "main"], in: repoRoot)

        // Merge --squash
        _ = try await runGitCommand(["merge", "--squash", trigger.mainBranch], in: repoRoot)

        // Commit with generated message
        let commitMessage = "lfd(\(trigger.areaDisplay)): land \(trigger.commitsAhead) commits"
        _ = try await runGitCommand(["commit", "-m", commitMessage], in: repoRoot)

        // Push
        _ = try await runGitCommand(["push"], in: repoRoot)
    }

    public func connectLfd() async throws {
        try await runShellCommand(["lfd", "install"])
    }

    // MARK: - Private helpers

    private func getCurrentRun(db: OpaquePointer?, parent: String) -> (id: String, currentStep: String?)? {
        let query = """
            SELECT id, current_step FROM runs
            WHERE parent = ? AND status = 'running'
            ORDER BY started_at DESC LIMIT 1
        """

        var stmt: OpaquePointer?
        guard sqlite3_prepare_v2(db, query, -1, &stmt, nil) == SQLITE_OK else {
            return nil
        }
        defer { sqlite3_finalize(stmt) }

        sqlite3_bind_text(stmt, 1, parent, -1, nil)

        if sqlite3_step(stmt) == SQLITE_ROW,
           let idPtr = sqlite3_column_text(stmt, 0) {
            let id = String(cString: idPtr)
            let currentStep = sqlite3_column_text(stmt, 1).map { String(cString: $0) }
            return (id, currentStep)
        }

        return nil
    }

    private func decodeGoals(_ goalsStr: String?) -> [String] {
        guard let goalsStr, !goalsStr.isEmpty else { return [] }
        guard let data = goalsStr.data(using: .utf8) else { return [goalsStr] }
        if let decoded = try? JSONDecoder().decode([String].self, from: data) {
            return decoded
        }
        if let decoded = try? JSONDecoder().decode(String.self, from: data) {
            return [decoded]
        }
        return [goalsStr]
    }

    private func columnText(_ stmt: OpaquePointer?, _ index: Int32) -> String? {
        guard let ptr = sqlite3_column_text(stmt, index) else { return nil }
        return String(cString: ptr)
    }

    private func getCommitsAhead(branch: String, in repoRoot: URL) async throws -> Int {
        let result = try await runGitCommand(
            ["rev-list", "--count", "main..\(branch)"],
            in: repoRoot
        )
        return Int(result.trimmingCharacters(in: .whitespacesAndNewlines)) ?? 0
    }

    private func runGitCommand(_ args: [String], in directory: URL) async throws -> String {
        try await withCheckedThrowingContinuation { continuation in
            let process = Process()
            process.executableURL = URL(fileURLWithPath: "/usr/bin/git")
            process.arguments = args
            process.currentDirectoryURL = directory

            let pipe = Pipe()
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
                    continuation.resume(throwing: LfdServiceError.gitCommandFailed(output))
                }
            } catch {
                continuation.resume(throwing: error)
            }
        }
    }

    private func runShellCommand(_ args: [String]) async throws {
        try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Void, Error>) in
            let process = Process()
            process.executableURL = URL(fileURLWithPath: "/bin/zsh")
            process.arguments = ["-l", "-c", args.joined(separator: " ")]

            do {
                try process.run()
                process.waitUntilExit()
                if process.terminationStatus == 0 {
                    continuation.resume()
                } else {
                    continuation.resume(throwing: LfdServiceError.commandFailed)
                }
            } catch {
                continuation.resume(throwing: error)
            }
        }
    }
}

public enum LfdServiceError: LocalizedError {
    case nothingToLand
    case gitCommandFailed(String)
    case commandFailed

    public var errorDescription: String? {
        switch self {
        case .nothingToLand:
            return "No commits to land"
        case .gitCommandFailed(let output):
            return "Git command failed: \(output)"
        case .commandFailed:
            return "Command failed"
        }
    }
}

// Backwards compatibility aliases
public typealias JobService = LfdService
public typealias JobServiceError = LfdServiceError
public typealias LoopService = LfdService
public typealias LoopServiceError = LfdServiceError
