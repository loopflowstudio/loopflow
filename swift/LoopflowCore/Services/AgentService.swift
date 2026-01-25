// Service for loading Agent and FlowRun data from lfd.db SQLite database.

import Foundation
import SQLite3

public struct AgentService: @unchecked Sendable {
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
            SELECT id, name, flow, goal, area, repo, status, iteration,
                   worktree, branch, pr_limit, merge_mode, pid, created_at,
                   stimulus_kind, stimulus_cron, last_main_sha
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
                  let flow = columnText(stmt, 2),
                  let repoPath = columnText(stmt, 5),
                  let statusStr = columnText(stmt, 6) else {
                continue
            }

            let name = columnText(stmt, 1) ?? ""
            let goal = decodeStringArray(columnText(stmt, 3))
            let area = decodeStringArray(columnText(stmt, 4))
            let iteration = Int(sqlite3_column_int(stmt, 7))
            let worktreePath = columnText(stmt, 8)
            let branch = columnText(stmt, 9)
            let prLimit = Int(sqlite3_column_int(stmt, 10))
            let mergeModeStr = columnText(stmt, 11) ?? "pr"
            let pid = sqlite3_column_type(stmt, 12) != SQLITE_NULL
                ? Int(sqlite3_column_int(stmt, 12))
                : nil

            var createdAt = Date()
            if let dateStr = columnText(stmt, 13) {
                createdAt = dateFormatter.date(from: dateStr) ?? Date()
            }

            let stimulusKindStr = columnText(stmt, 14) ?? "loop"
            let stimulusCron = columnText(stmt, 15)
            let lastMainSha = columnText(stmt, 16)

            let status = AgentStatus(rawValue: statusStr) ?? .idle
            let mergeMode = MergeMode(rawValue: mergeModeStr) ?? .pr
            let stimulusKind = Stimulus.Kind(rawValue: stimulusKindStr) ?? .loop
            let stimulus = Stimulus(kind: stimulusKind, cron: stimulusCron)

            let agent = Agent(
                id: id,
                name: name,
                flow: flow,
                goal: goal,
                area: area,
                repo: repoPath,
                stimulus: stimulus,
                status: status,
                iteration: iteration,
                worktreePath: worktreePath,
                branch: branch,
                prLimit: prLimit,
                mergeMode: mergeMode,
                pid: pid,
                createdAt: createdAt,
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
            SELECT id, agent_id, flow, area, repo, goal, status, iteration,
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
            let goal = decodeStringArray(columnText(stmt, 5))
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
                goal: goal,
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

    public func createAgent(name: String, description: String, area: String, repo: URL) async throws -> Agent {
        // Create agent via lfd HTTP API
        let baseURL = URL(string: "http://127.0.0.1:8765")!
        var components = URLComponents(url: baseURL.appendingPathComponent("agents"), resolvingAgainstBaseURL: false)!
        components.queryItems = [URLQueryItem(name: "repo", value: repo.path)]

        var request = URLRequest(url: components.url!)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")

        let body: [String: Any] = [
            "name": name.isEmpty ? NSNull() : name,
            "flow": "ship",
            "goal": ["default"],
            "area": area.isEmpty ? ["."] : [area]
        ]
        request.httpBody = try JSONSerialization.data(withJSONObject: body)

        let (data, response) = try await URLSession.shared.data(for: request)

        guard let httpResponse = response as? HTTPURLResponse, httpResponse.statusCode == 200 else {
            let statusCode = (response as? HTTPURLResponse)?.statusCode ?? 0
            throw AgentServiceError.commandFailed("HTTP \(statusCode)")
        }

        guard let json = try JSONSerialization.jsonObject(with: data) as? [String: Any],
              let ok = json["ok"] as? Bool, ok,
              let result = json["result"] as? [String: Any],
              let agentData = result["agent"] as? [String: Any] else {
            let errorMsg = (try? JSONSerialization.jsonObject(with: data) as? [String: Any])?["error"] as? String
            throw AgentServiceError.commandFailed(errorMsg ?? "Invalid response")
        }

        return parseAgentFromJSON(agentData)
    }

    private func parseAgentFromJSON(_ json: [String: Any]) -> Agent {
        let dateFormatter = ISO8601DateFormatter()
        dateFormatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]

        let stimulus: Stimulus
        if let stimDict = json["stimulus"] as? [String: Any] {
            let kind = Stimulus.Kind(rawValue: stimDict["kind"] as? String ?? "once") ?? .once
            stimulus = Stimulus(kind: kind, cron: stimDict["cron"] as? String)
        } else {
            stimulus = Stimulus(kind: .once)
        }

        var createdAt = Date()
        if let dateStr = json["created_at"] as? String {
            createdAt = dateFormatter.date(from: dateStr) ?? Date()
        }

        return Agent(
            id: json["id"] as? String ?? UUID().uuidString,
            name: json["name"] as? String ?? "",
            flow: json["flow"] as? String ?? "ship",
            goal: json["goal"] as? [String] ?? [],
            area: json["area"] as? [String] ?? ["."],
            repo: json["repo"] as? String ?? "",
            stimulus: stimulus,
            status: AgentStatus(rawValue: json["status"] as? String ?? "idle") ?? .idle,
            iteration: json["iteration"] as? Int ?? 0,
            worktreePath: json["worktree"] as? String,
            branch: json["branch"] as? String,
            prLimit: json["pr_limit"] as? Int ?? 5,
            mergeMode: MergeMode(rawValue: json["merge_mode"] as? String ?? "pr") ?? .pr,
            pid: json["pid"] as? Int,
            createdAt: createdAt
        )
    }

    public func runFlow(agentId: String, flow: String, stimulus: Stimulus.Kind, args: String, repo: URL) async throws {
        // Run flow for agent via lfd
        var command = ["lfd"]
        switch stimulus {
        case .once:
            command.append("run")
        case .loop:
            command.append("loop")
        case .watch:
            command.append("subscribe")
        case .cron:
            command.append("schedule")
        case .manual:
            command.append("run")
        }
        command.append(flow)
        command.append(".")  // area - would come from agent

        try await runShellCommand(command)
    }

    public func stopAgent(agentId: String) async throws {
        try await runShellCommand(["lfd", "stop", agentId])
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

            let pipe = Pipe()
            process.standardOutput = pipe
            process.standardError = pipe

            do {
                try process.run()
                process.waitUntilExit()

                let data = pipe.fileHandleForReading.readDataToEndOfFile()
                let output = String(data: data, encoding: .utf8)?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""

                if process.terminationStatus == 0 {
                    continuation.resume()
                } else {
                    continuation.resume(throwing: AgentServiceError.commandFailed(output.isEmpty ? "Exit code \(process.terminationStatus)" : output))
                }
            } catch {
                continuation.resume(throwing: error)
            }
        }
    }
}

public enum AgentServiceError: LocalizedError {
    case commandFailed(String)

    public var errorDescription: String? {
        switch self {
        case .commandFailed(let message):
            return message
        }
    }
}
