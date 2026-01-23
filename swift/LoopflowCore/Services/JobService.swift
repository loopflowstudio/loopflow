// Service for loading jobs from lfd.db SQLite database.

import Foundation
import SQLite3

public struct JobService: @unchecked Sendable {
    public init() {}
    private let dbPath = FileManager.default.homeDirectoryForCurrentUser
        .appendingPathComponent(".lf/lfd.db").path

    public func listJobs(repo: URL) async throws -> [Job] {
        var jobs: [Job] = []

        var db: OpaquePointer?
        guard sqlite3_open(dbPath, &db) == SQLITE_OK else {
            return []
        }
        defer { sqlite3_close(db) }

        // Detect table name (jobs vs loops for migration compatibility)
        let tableName = detectTableName(db: db)
        let mainColumn = tableName == "jobs" ? "job_main" : "loop_main"

        let columns = ensureJobColumns(db: db, tableName: tableName)
        var selectColumns = ["id", "type", "area", "goals", "flow"]
        if columns.contains("goal") {
            selectColumns.append("goal")
        }
        selectColumns.append(contentsOf: [
            "repo",
            mainColumn,
            "status",
            "iteration",
            "pr_limit",
            "merge_mode",
            "pid",
            "created_at",
        ])

        let query = """
            SELECT \(selectColumns.joined(separator: ", "))
            FROM \(tableName)
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

        let columnIndex = Dictionary(uniqueKeysWithValues: selectColumns.enumerated().map { ($1, $0) })

        while sqlite3_step(stmt) == SQLITE_ROW {
            guard let id = columnText(stmt, columnIndex["id"]),
                  let typeStr = columnText(stmt, columnIndex["type"]),
                  let repoPath = columnText(stmt, columnIndex["repo"]),
                  let jobMain = columnText(stmt, columnIndex[mainColumn]),
                  let statusStr = columnText(stmt, columnIndex["status"]) else {
                continue
            }

            let areaValue = columnText(stmt, columnIndex["area"]) ?? "."
            let goalFallback = columnText(stmt, columnIndex["goal"])
            var goals = decodeGoals(columnText(stmt, columnIndex["goals"]))
            if goals.isEmpty, let goalFallback, !goalFallback.isEmpty {
                goals = [goalFallback]
            }
            let flow = columnText(stmt, columnIndex["flow"])
            let area = areaValue.isEmpty ? "." : areaValue

            let iterationIndex = Int32(columnIndex["iteration"] ?? 0)
            let prLimitIndex = Int32(columnIndex["pr_limit"] ?? 0)
            let iteration = Int(sqlite3_column_int(stmt, iterationIndex))
            let prLimit = Int(sqlite3_column_int(stmt, prLimitIndex))

            let mergeModeStr = columnText(stmt, columnIndex["merge_mode"]) ?? "pr"
            let pidIndex = Int32(columnIndex["pid"] ?? 0)
            let pid = sqlite3_column_type(stmt, pidIndex) != SQLITE_NULL
                ? Int(sqlite3_column_int(stmt, pidIndex))
                : nil

            var createdAt = Date()
            if let dateStr = columnText(stmt, columnIndex["created_at"]) {
                createdAt = dateFormatter.date(from: dateStr) ?? ISO8601DateFormatter().date(from: dateStr) ?? Date()
            }

            let type = JobType(rawValue: typeStr) ?? .loop
            let status = JobStatus(rawValue: statusStr) ?? .idle
            let mergeMode = JobMergeMode(rawValue: mergeModeStr) ?? .pr

            var job = Job(
                id: id,
                type: type,
                area: area,
                goals: goals,
                flow: flow,
                repo: repoPath,
                jobMain: jobMain,
                status: status,
                iteration: iteration,
                prLimit: prLimit,
                mergeMode: mergeMode,
                pid: pid,
                createdAt: createdAt
            )

            // Get current run info if running
            if status == .running {
                if let runInfo = getCurrentRun(db: db, jobId: id, tableName: tableName) {
                    job.currentRunId = runInfo.id
                    job.currentStep = runInfo.currentStep
                }
            }

            // Check commits ahead of main
            if let commits = try? await getCommitsAhead(branch: jobMain, in: URL(fileURLWithPath: repoPath)) {
                job.commitsAhead = commits
            }

            jobs.append(job)
        }

        return jobs
    }

    private func detectTableName(db: OpaquePointer?) -> String {
        guard let db else { return "loops" }
        var stmt: OpaquePointer?
        guard sqlite3_prepare_v2(
            db,
            "SELECT name FROM sqlite_master WHERE type='table' AND name IN ('jobs', 'loops')",
            -1,
            &stmt,
            nil
        ) == SQLITE_OK else {
            return "loops"
        }
        defer { sqlite3_finalize(stmt) }

        var tables: Set<String> = []
        while sqlite3_step(stmt) == SQLITE_ROW {
            if let namePtr = sqlite3_column_text(stmt, 0) {
                tables.insert(String(cString: namePtr))
            }
        }
        return tables.contains("jobs") ? "jobs" : "loops"
    }

    private func ensureJobColumns(db: OpaquePointer?, tableName: String) -> Set<String> {
        guard let db else { return [] }
        var stmt: OpaquePointer?
        guard sqlite3_prepare_v2(db, "PRAGMA table_info(\(tableName))", -1, &stmt, nil) == SQLITE_OK else {
            return []
        }
        defer { sqlite3_finalize(stmt) }

        var columns: Set<String> = []
        while sqlite3_step(stmt) == SQLITE_ROW {
            if let namePtr = sqlite3_column_text(stmt, 1) {
                columns.insert(String(cString: namePtr))
            }
        }

        if !columns.contains("area") {
            sqlite3_exec(db, "ALTER TABLE \(tableName) ADD COLUMN area TEXT", nil, nil, nil)
            columns.insert("area")
        }
        if !columns.contains("goals") {
            sqlite3_exec(db, "ALTER TABLE \(tableName) ADD COLUMN goals TEXT", nil, nil, nil)
            columns.insert("goals")
        }
        if !columns.contains("flow") {
            sqlite3_exec(db, "ALTER TABLE \(tableName) ADD COLUMN flow TEXT", nil, nil, nil)
            columns.insert("flow")
        }
        return columns
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

    private func columnText(_ stmt: OpaquePointer?, _ index: Int?) -> String? {
        guard let index else { return nil }
        guard let ptr = sqlite3_column_text(stmt, Int32(index)) else { return nil }
        return String(cString: ptr)
    }

    private func getCommitsAhead(branch: String, in repoRoot: URL) async throws -> Int {
        let result = try await runGitCommand(
            ["rev-list", "--count", "main..\(branch)"],
            in: repoRoot
        )
        return Int(result.trimmingCharacters(in: .whitespacesAndNewlines)) ?? 0
    }

    public func getJobRuns(jobId: String, limit: Int = 10) async throws -> [JobRun] {
        var runs: [JobRun] = []

        var db: OpaquePointer?
        guard sqlite3_open(dbPath, &db) == SQLITE_OK else {
            return []
        }
        defer { sqlite3_close(db) }

        // Detect table name for migration compatibility
        let runsTable = detectRunsTableName(db: db)
        let idColumn = runsTable == "job_runs" ? "job_id" : "loop_id"

        let query = """
            SELECT id, \(idColumn), iteration, status, started_at, ended_at,
                   worktree, current_step, error, pr_url
            FROM \(runsTable)
            WHERE \(idColumn) = ? OR \(idColumn) LIKE ?
            ORDER BY started_at DESC
            LIMIT ?
        """

        var stmt: OpaquePointer?
        guard sqlite3_prepare_v2(db, query, -1, &stmt, nil) == SQLITE_OK else {
            return []
        }
        defer { sqlite3_finalize(stmt) }

        sqlite3_bind_text(stmt, 1, jobId, -1, nil)
        sqlite3_bind_text(stmt, 2, "\(jobId)%", -1, nil)
        sqlite3_bind_int(stmt, 3, Int32(limit))

        let dateFormatter = ISO8601DateFormatter()
        dateFormatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]

        while sqlite3_step(stmt) == SQLITE_ROW {
            guard let idPtr = sqlite3_column_text(stmt, 0),
                  let jobIdPtr = sqlite3_column_text(stmt, 1),
                  let statusPtr = sqlite3_column_text(stmt, 3),
                  let startedAtPtr = sqlite3_column_text(stmt, 4) else {
                continue
            }

            let id = String(cString: idPtr)
            let runJobId = String(cString: jobIdPtr)
            let iteration = Int(sqlite3_column_int(stmt, 2))
            let statusStr = String(cString: statusPtr)
            let startedAtStr = String(cString: startedAtPtr)

            let status = JobStatus(rawValue: statusStr) ?? .idle
            let startedAt = dateFormatter.date(from: startedAtStr) ?? Date()

            var endedAt: Date?
            if let endedAtPtr = sqlite3_column_text(stmt, 5) {
                endedAt = dateFormatter.date(from: String(cString: endedAtPtr))
            }

            let worktree = sqlite3_column_text(stmt, 6).map { String(cString: $0) }
            let currentStep = sqlite3_column_text(stmt, 7).map { String(cString: $0) }
            let error = sqlite3_column_text(stmt, 8).map { String(cString: $0) }
            let prUrl = sqlite3_column_text(stmt, 9).map { String(cString: $0) }

            runs.append(JobRun(
                id: id,
                jobId: runJobId,
                iteration: iteration,
                status: status,
                startedAt: startedAt,
                endedAt: endedAt,
                worktree: worktree,
                currentStep: currentStep,
                error: error,
                prUrl: prUrl
            ))
        }

        return runs
    }

    private func detectRunsTableName(db: OpaquePointer?) -> String {
        guard let db else { return "loop_runs" }
        var stmt: OpaquePointer?
        guard sqlite3_prepare_v2(
            db,
            "SELECT name FROM sqlite_master WHERE type='table' AND name IN ('job_runs', 'loop_runs')",
            -1,
            &stmt,
            nil
        ) == SQLITE_OK else {
            return "loop_runs"
        }
        defer { sqlite3_finalize(stmt) }

        var tables: Set<String> = []
        while sqlite3_step(stmt) == SQLITE_ROW {
            if let namePtr = sqlite3_column_text(stmt, 0) {
                tables.insert(String(cString: namePtr))
            }
        }
        return tables.contains("job_runs") ? "job_runs" : "loop_runs"
    }

    public func squashLand(job: Job, repoRoot: URL) async throws {
        guard job.commitsAhead > 0 else {
            throw JobServiceError.nothingToLand
        }

        // Checkout main
        _ = try await runGitCommand(["checkout", "main"], in: repoRoot)

        // Merge --squash
        _ = try await runGitCommand(["merge", "--squash", job.jobMain], in: repoRoot)

        // Commit with generated message
        let commitMessage = "job(\(job.areaDisplay)): land \(job.commitsAhead) commits"
        _ = try await runGitCommand(["commit", "-m", commitMessage], in: repoRoot)

        // Push
        _ = try await runGitCommand(["push"], in: repoRoot)
    }

    public func connectLfd() async throws {
        try await runShellCommand(["lfd", "install"])
    }

    private func getCurrentRun(db: OpaquePointer?, jobId: String, tableName: String) -> (id: String, currentStep: String?)? {
        let runsTable = tableName == "jobs" ? "job_runs" : "loop_runs"
        let idColumn = tableName == "jobs" ? "job_id" : "loop_id"

        let query = """
            SELECT id, current_step FROM \(runsTable)
            WHERE \(idColumn) = ? AND status = 'running'
            ORDER BY started_at DESC LIMIT 1
        """

        var stmt: OpaquePointer?
        guard sqlite3_prepare_v2(db, query, -1, &stmt, nil) == SQLITE_OK else {
            return nil
        }
        defer { sqlite3_finalize(stmt) }

        sqlite3_bind_text(stmt, 1, jobId, -1, nil)

        if sqlite3_step(stmt) == SQLITE_ROW,
           let idPtr = sqlite3_column_text(stmt, 0) {
            let id = String(cString: idPtr)
            let currentStep = sqlite3_column_text(stmt, 1).map { String(cString: $0) }
            return (id, currentStep)
        }

        return nil
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
                    continuation.resume(throwing: JobServiceError.gitCommandFailed(output))
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
                    continuation.resume(throwing: JobServiceError.commandFailed)
                }
            } catch {
                continuation.resume(throwing: error)
            }
        }
    }

    // Backwards compatibility methods
    public func listLoops(repo: URL) async throws -> [Job] {
        try await listJobs(repo: repo)
    }

    public func getLoopRuns(loopId: String, limit: Int = 10) async throws -> [JobRun] {
        try await getJobRuns(jobId: loopId, limit: limit)
    }

    public func squashLand(loop: Job, repoRoot: URL) async throws {
        try await squashLand(job: loop, repoRoot: repoRoot)
    }
}

public enum JobServiceError: LocalizedError {
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

// Backwards compatibility alias
public typealias LoopService = JobService
public typealias LoopServiceError = JobServiceError
