// Service for loading loops from lfd.db SQLite database.

import Foundation
import SQLite3

struct LoopService {
    private let dbPath = FileManager.default.homeDirectoryForCurrentUser
        .appendingPathComponent(".lf/lfd.db").path

    func listLoops(repo: URL) async throws -> [Loop] {
        var loops: [Loop] = []

        var db: OpaquePointer?
        guard sqlite3_open(dbPath, &db) == SQLITE_OK else {
            return []
        }
        defer { sqlite3_close(db) }

        let query = """
            SELECT id, type, goal, repo, loop_main, status, iteration, pr_limit,
                   merge_mode, pid, created_at
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
            guard let idPtr = sqlite3_column_text(stmt, 0),
                  let typePtr = sqlite3_column_text(stmt, 1),
                  let goalPtr = sqlite3_column_text(stmt, 2),
                  let repoPtr = sqlite3_column_text(stmt, 3),
                  let loopMainPtr = sqlite3_column_text(stmt, 4),
                  let statusPtr = sqlite3_column_text(stmt, 5) else {
                continue
            }

            let id = String(cString: idPtr)
            let typeStr = String(cString: typePtr)
            let goal = String(cString: goalPtr)
            let repoPath = String(cString: repoPtr)
            let loopMain = String(cString: loopMainPtr)
            let statusStr = String(cString: statusPtr)

            let iteration = Int(sqlite3_column_int(stmt, 6))
            let prLimit = Int(sqlite3_column_int(stmt, 7))

            let mergeModeStr = sqlite3_column_text(stmt, 8).map { String(cString: $0) } ?? "pr"
            let pid = sqlite3_column_type(stmt, 9) != SQLITE_NULL ? Int(sqlite3_column_int(stmt, 9)) : nil

            var createdAt = Date()
            if let datePtr = sqlite3_column_text(stmt, 10) {
                let dateStr = String(cString: datePtr)
                createdAt = dateFormatter.date(from: dateStr) ?? ISO8601DateFormatter().date(from: dateStr) ?? Date()
            }

            let type = LoopType(rawValue: typeStr) ?? .loop
            let status = LoopStatus(rawValue: statusStr) ?? .idle
            let mergeMode = LoopMergeMode(rawValue: mergeModeStr) ?? .pr

            var loop = Loop(
                id: id,
                type: type,
                goalName: goal,
                repo: repoPath,
                loopMain: loopMain,
                status: status,
                iteration: iteration,
                prLimit: prLimit,
                mergeMode: mergeMode,
                pid: pid,
                createdAt: createdAt
            )

            // Get current run info if running
            if status == .running {
                if let runInfo = getCurrentRun(db: db, loopId: id) {
                    loop.currentRunId = runInfo.id
                    loop.currentStep = runInfo.currentStep
                }
            }

            // Check commits ahead of main
            if let commits = try? await getCommitsAhead(branch: loopMain, in: URL(fileURLWithPath: repoPath)) {
                loop.commitsAhead = commits
            }

            loops.append(loop)
        }

        return loops
    }

    private func getCommitsAhead(branch: String, in repoRoot: URL) async throws -> Int {
        let result = try await runGitCommand(
            ["rev-list", "--count", "main..\(branch)"],
            in: repoRoot
        )
        return Int(result.trimmingCharacters(in: .whitespacesAndNewlines)) ?? 0
    }

    func getLoopRuns(loopId: String, limit: Int = 10) async throws -> [LoopRun] {
        var runs: [LoopRun] = []

        var db: OpaquePointer?
        guard sqlite3_open(dbPath, &db) == SQLITE_OK else {
            return []
        }
        defer { sqlite3_close(db) }

        let query = """
            SELECT id, loop_id, iteration, status, started_at, ended_at,
                   worktree, current_step, error, pr_url
            FROM loop_runs
            WHERE loop_id = ? OR loop_id LIKE ?
            ORDER BY started_at DESC
            LIMIT ?
        """

        var stmt: OpaquePointer?
        guard sqlite3_prepare_v2(db, query, -1, &stmt, nil) == SQLITE_OK else {
            return []
        }
        defer { sqlite3_finalize(stmt) }

        sqlite3_bind_text(stmt, 1, loopId, -1, nil)
        sqlite3_bind_text(stmt, 2, "\(loopId)%", -1, nil)
        sqlite3_bind_int(stmt, 3, Int32(limit))

        let dateFormatter = ISO8601DateFormatter()
        dateFormatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]

        while sqlite3_step(stmt) == SQLITE_ROW {
            guard let idPtr = sqlite3_column_text(stmt, 0),
                  let loopIdPtr = sqlite3_column_text(stmt, 1),
                  let statusPtr = sqlite3_column_text(stmt, 3),
                  let startedAtPtr = sqlite3_column_text(stmt, 4) else {
                continue
            }

            let id = String(cString: idPtr)
            let runLoopId = String(cString: loopIdPtr)
            let iteration = Int(sqlite3_column_int(stmt, 2))
            let statusStr = String(cString: statusPtr)
            let startedAtStr = String(cString: startedAtPtr)

            let status = LoopStatus(rawValue: statusStr) ?? .idle
            let startedAt = dateFormatter.date(from: startedAtStr) ?? Date()

            var endedAt: Date?
            if let endedAtPtr = sqlite3_column_text(stmt, 5) {
                endedAt = dateFormatter.date(from: String(cString: endedAtPtr))
            }

            let worktree = sqlite3_column_text(stmt, 6).map { String(cString: $0) }
            let currentStep = sqlite3_column_text(stmt, 7).map { String(cString: $0) }
            let error = sqlite3_column_text(stmt, 8).map { String(cString: $0) }
            let prUrl = sqlite3_column_text(stmt, 9).map { String(cString: $0) }

            runs.append(LoopRun(
                id: id,
                loopId: runLoopId,
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

    func squashLand(loop: Loop, repoRoot: URL) async throws {
        guard loop.commitsAhead > 0 else {
            throw LoopServiceError.nothingToLand
        }

        // Checkout main
        _ = try await runGitCommand(["checkout", "main"], in: repoRoot)

        // Merge --squash
        _ = try await runGitCommand(["merge", "--squash", loop.loopMain], in: repoRoot)

        // Commit with generated message
        let commitMessage = "loop(\(loop.goalName)): land \(loop.commitsAhead) commits"
        _ = try await runGitCommand(["commit", "-m", commitMessage], in: repoRoot)

        // Push
        _ = try await runGitCommand(["push"], in: repoRoot)
    }

    func connectLfd() async throws {
        try await runShellCommand(["lfd", "install"])
    }

    private func getCurrentRun(db: OpaquePointer?, loopId: String) -> (id: String, currentStep: String?)? {
        let query = """
            SELECT id, current_step FROM loop_runs
            WHERE loop_id = ? AND status = 'running'
            ORDER BY started_at DESC LIMIT 1
        """

        var stmt: OpaquePointer?
        guard sqlite3_prepare_v2(db, query, -1, &stmt, nil) == SQLITE_OK else {
            return nil
        }
        defer { sqlite3_finalize(stmt) }

        sqlite3_bind_text(stmt, 1, loopId, -1, nil)

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
                    continuation.resume(throwing: LoopServiceError.gitCommandFailed(output))
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
                    continuation.resume(throwing: LoopServiceError.commandFailed)
                }
            } catch {
                continuation.resume(throwing: error)
            }
        }
    }
}

enum LoopServiceError: LocalizedError {
    case nothingToLand
    case gitCommandFailed(String)
    case commandFailed

    var errorDescription: String? {
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
