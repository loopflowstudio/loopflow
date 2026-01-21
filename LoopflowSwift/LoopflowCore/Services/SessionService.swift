// Service for loading task session history from lfd.db.

import Foundation
import SQLite3

// SQLITE_TRANSIENT tells SQLite to copy the string immediately
private let SQLITE_TRANSIENT = unsafeBitCast(-1, to: sqlite3_destructor_type.self)

public struct SessionService: @unchecked Sendable {
    public init() {}
    private let dbPath = FileManager.default.homeDirectoryForCurrentUser
        .appendingPathComponent(".lf/lfd.db").path

    public func history(for worktree: String, limit: Int = 20) async throws -> [TaskSession] {
        return loadSessions(worktree: worktree, limit: limit)
    }

    public func historyForRepo(_ repo: String, limit: Int = 50) async throws -> [TaskSession] {
        return loadSessions(repo: repo, limit: limit)
    }

    private func loadSessions(worktree: String? = nil, repo: String? = nil, limit: Int = 20) -> [TaskSession] {
        var sessions: [TaskSession] = []

        var db: OpaquePointer?
        guard sqlite3_open(dbPath, &db) == SQLITE_OK else {
            return sessions
        }
        defer { sqlite3_close(db) }

        var query: String
        var bindValue: String?

        if let worktree = worktree {
            query = """
                SELECT id, task, repo, worktree, status, started_at, ended_at, model, run_mode
                FROM sessions
                WHERE worktree = ?
                ORDER BY started_at DESC
                LIMIT \(limit)
            """
            bindValue = worktree
        } else if let repo = repo {
            query = """
                SELECT id, task, repo, worktree, status, started_at, ended_at, model, run_mode
                FROM sessions
                WHERE repo = ?
                ORDER BY started_at DESC
                LIMIT \(limit)
            """
            bindValue = repo
        } else {
            query = """
                SELECT id, task, repo, worktree, status, started_at, ended_at, model, run_mode
                FROM sessions
                ORDER BY started_at DESC
                LIMIT \(limit)
            """
        }

        var stmt: OpaquePointer?
        guard sqlite3_prepare_v2(db, query, -1, &stmt, nil) == SQLITE_OK else {
            return sessions
        }
        defer { sqlite3_finalize(stmt) }

        if let value = bindValue {
            sqlite3_bind_text(stmt, 1, value, -1, SQLITE_TRANSIENT)
        }

        let dateFormatter = ISO8601DateFormatter()
        dateFormatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]

        while sqlite3_step(stmt) == SQLITE_ROW {
            guard let idPtr = sqlite3_column_text(stmt, 0),
                  let taskPtr = sqlite3_column_text(stmt, 1),
                  let repoPtr = sqlite3_column_text(stmt, 2),
                  let worktreePtr = sqlite3_column_text(stmt, 3),
                  let statusPtr = sqlite3_column_text(stmt, 4),
                  let startedAtPtr = sqlite3_column_text(stmt, 5) else {
                continue
            }

            let id = String(cString: idPtr)
            let task = String(cString: taskPtr)
            let repo = String(cString: repoPtr)
            let worktree = String(cString: worktreePtr)
            let status = String(cString: statusPtr)
            let startedAtStr = String(cString: startedAtPtr)

            guard let startedAt = dateFormatter.date(from: startedAtStr) ?? ISO8601DateFormatter().date(from: startedAtStr) else {
                continue
            }

            var endedAt: Date?
            if sqlite3_column_type(stmt, 6) != SQLITE_NULL,
               let endedAtPtr = sqlite3_column_text(stmt, 6) {
                let endedAtStr = String(cString: endedAtPtr)
                endedAt = dateFormatter.date(from: endedAtStr) ?? ISO8601DateFormatter().date(from: endedAtStr)
            }

            let model = sqlite3_column_text(stmt, 7).map { String(cString: $0) } ?? "claude-code"
            let runMode = sqlite3_column_text(stmt, 8).map { String(cString: $0) } ?? "auto"

            sessions.append(TaskSession(
                id: id,
                task: task,
                repo: repo,
                worktree: worktree,
                status: status,
                startedAt: startedAt,
                endedAt: endedAt,
                model: model,
                runMode: runMode
            ))
        }

        return sessions
    }
}
