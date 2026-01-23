// Service for loading data from lfd.db SQLite database.

import Foundation
import SQLite3

public struct LfdService: @unchecked Sendable {
    public init() {}
    private let dbPath = FileManager.default.homeDirectoryForCurrentUser
        .appendingPathComponent(".lf/lfd.db").path

    // MARK: - Agents

    public func listAgents(repo: URL) async throws -> [Agent] {
        var agents: [Agent] = []

        var db: OpaquePointer?
        guard sqlite3_open(dbPath, &db) == SQLITE_OK else {
            return []
        }
        defer { sqlite3_close(db) }

        let query = """
            SELECT id, flow, voice, area, repo, status, iteration,
                   main_branch, pr_limit, merge_mode, pid, created_at,
                   watch_paths, cron, last_main_sha
            FROM agents
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
                  let repoPath = columnText(stmt, 4),
                  let statusStr = columnText(stmt, 5),
                  let mainBranch = columnText(stmt, 7) else {
                continue
            }

            let voice = decodeStringArray(columnText(stmt, 2))
            let area = decodeStringArray(columnText(stmt, 3))
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

            let watchPaths = columnText(stmt, 12)
            let cron = columnText(stmt, 13)
            let lastMainSha = columnText(stmt, 14)

            let status = AgentStatus(rawValue: statusStr) ?? .idle
            let mergeMode = MergeMode(rawValue: mergeModeStr) ?? .pr

            let agent = Agent(
                id: id,
                flow: flow,
                voice: voice,
                area: area,
                repo: repoPath,
                status: status,
                iteration: iteration,
                mainBranch: mainBranch,
                prLimit: prLimit,
                mergeMode: mergeMode,
                pid: pid,
                createdAt: createdAt,
                watchPaths: watchPaths,
                cron: cron,
                lastMainSha: lastMainSha
            )

            agents.append(agent)
        }

        return agents
    }

    // MARK: - FlowRuns

    public func listFlowRuns(agentId: String? = nil, repo: URL? = nil, limit: Int = 50) async throws -> [FlowRun] {
        var flowRuns: [FlowRun] = []

        var db: OpaquePointer?
        guard sqlite3_open(dbPath, &db) == SQLITE_OK else {
            return []
        }
        defer { sqlite3_close(db) }

        var query = """
            SELECT id, agent_id, flow, area, repo, voice, status, iteration,
                   worktree, branch, current_step, error, pr_url,
                   started_at, ended_at, created_at
            FROM runs
        """

        var conditions: [String] = []
        if agentId != nil {
            conditions.append("agent_id = ?")
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
        if let agentId {
            sqlite3_bind_text(stmt, paramIndex, agentId, -1, nil)
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

            let agentId = columnText(stmt, 1)
            let area = columnText(stmt, 3) ?? "."
            let voice = decodeStringArray(columnText(stmt, 5))
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

            let status = FlowRunStatus(rawValue: statusStr) ?? .pending

            flowRuns.append(FlowRun(
                id: id,
                agentId: agentId,
                flow: flow,
                area: area,
                repo: repoPath,
                voice: voice,
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

        return flowRuns
    }

    public func getFlowRuns(forAgent agentId: String, limit: Int = 10) async throws -> [FlowRun] {
        return try await listFlowRuns(agentId: agentId, limit: limit)
    }

    // MARK: - Actions

    public func connectLfd() async throws {
        try await runShellCommand(["lfd", "install"])
    }

    // MARK: - Private helpers

    private func decodeStringArray(_ str: String?) -> [String] {
        guard let str, !str.isEmpty else { return [] }
        guard let data = str.data(using: .utf8) else { return [str] }
        if let decoded = try? JSONDecoder().decode([String].self, from: data) {
            return decoded
        }
        if let decoded = try? JSONDecoder().decode(String.self, from: data) {
            return [decoded]
        }
        return [str]
    }

    private func columnText(_ stmt: OpaquePointer?, _ index: Int32) -> String? {
        guard let ptr = sqlite3_column_text(stmt, index) else { return nil }
        return String(cString: ptr)
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
    case commandFailed

    public var errorDescription: String? {
        switch self {
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
