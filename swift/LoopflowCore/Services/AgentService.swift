// Service for loading Agent and FlowRun data from lfd.db SQLite database.

import Foundation
import SQLite3

public struct AgentService: @unchecked Sendable {
    public init() {}
    private let dbPath = FileManager.default.homeDirectoryForCurrentUser
        .appendingPathComponent(".lf/lfd.db").path

    // MARK: - Agents

    /// List agents for a repository via HTTP API.
    /// The API normalizes worktree paths to their main repo.
    public func listAgents(repo: URL) async throws -> [Agent] {
        let baseURL = URL(string: "http://127.0.0.1:8765")!
        var components = URLComponents(url: baseURL.appendingPathComponent("agents"), resolvingAgainstBaseURL: false)!
        components.queryItems = [URLQueryItem(name: "repo", value: repo.path)]

        guard let url = components.url else { return [] }

        do {
            let (data, response) = try await URLSession.shared.data(from: url)

            guard let httpResponse = response as? HTTPURLResponse, httpResponse.statusCode == 200 else {
                return []
            }

            guard let json = try JSONSerialization.jsonObject(with: data) as? [String: Any],
                  let ok = json["ok"] as? Bool, ok,
                  let result = json["result"] as? [String: Any],
                  let agentsData = result["agents"] as? [[String: Any]] else {
                return []
            }

            return agentsData.map { parseAgentFromJSON($0) }
        } catch {
            return []
        }
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

    public func createAgent(name: String, repo: URL) async throws -> Agent {
        // Create agent via lfd HTTP API with minimal config
        // User configures area, goal, flow in detail panel before running
        let baseURL = URL(string: "http://127.0.0.1:8765")!
        var components = URLComponents(url: baseURL.appendingPathComponent("agents"), resolvingAgainstBaseURL: false)!
        components.queryItems = [URLQueryItem(name: "repo", value: repo.path)]

        var request = URLRequest(url: components.url!)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")

        // Create with defaults - lfd requires non-empty goal/area
        let body: [String: Any] = [
            "name": name.isEmpty ? NSNull() : name,
            "flow": "design",  // Default flow (single step)
            "goal": ["default"],
            "area": ["."]  // Root directory
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
            area: json["area"] as? [String],  // nullable
            goal: json["goal"] as? [String],  // nullable
            flow: json["flow"] as? String ?? "design",
            repo: json["repo"] as? String ?? "",
            stimulus: stimulus,
            paused: json["paused"] as? Bool ?? true,
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

    /// Update agent configuration (area, goal, flow, stimulus, paused).
    public func updateAgent(
        agentId: String,
        area: [String]? = nil,
        goal: [String]? = nil,
        flow: String? = nil,
        stimulus: Stimulus? = nil,
        paused: Bool? = nil
    ) async throws -> Agent {
        let baseURL = URL(string: "http://127.0.0.1:8765")!
        let url = baseURL.appendingPathComponent("agents/\(agentId)")

        var request = URLRequest(url: url)
        request.httpMethod = "PATCH"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")

        var body: [String: Any] = [:]
        if let area = area { body["area"] = area }
        if let goal = goal { body["goal"] = goal }
        if let flow = flow { body["flow"] = flow }
        if let stimulus = stimulus {
            var stimDict: [String: Any] = ["kind": stimulus.kind.rawValue]
            if let cron = stimulus.cron { stimDict["cron"] = cron }
            body["stimulus"] = stimDict
        }
        if let paused = paused { body["paused"] = paused }

        request.httpBody = try JSONSerialization.data(withJSONObject: body)

        let (data, response) = try await URLSession.shared.data(for: request)

        guard let httpResponse = response as? HTTPURLResponse, httpResponse.statusCode == 200 else {
            let statusCode = (response as? HTTPURLResponse)?.statusCode ?? 0
            let errorMsg = (try? JSONSerialization.jsonObject(with: data) as? [String: Any])?["error"] as? String
            throw AgentServiceError.commandFailed(errorMsg ?? "HTTP \(statusCode)")
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

    /// Run agent, optionally with one-time overrides.
    public func run(
        agentId: String,
        area: [String]? = nil,
        goal: [String]? = nil,
        flow: String? = nil,
        stimulus: Stimulus? = nil
    ) async throws {
        let baseURL = URL(string: "http://127.0.0.1:8765")!
        let url = baseURL.appendingPathComponent("agents/\(agentId)/run")

        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")

        // Build body with any overrides
        var body: [String: Any] = [:]
        if let area = area { body["area"] = area }
        if let goal = goal { body["goal"] = goal }
        if let flow = flow { body["flow"] = flow }
        if let stimulus = stimulus {
            var stimDict: [String: Any] = ["kind": stimulus.kind.rawValue]
            if let cron = stimulus.cron { stimDict["cron"] = cron }
            body["stimulus"] = stimDict
        }

        if !body.isEmpty {
            request.httpBody = try JSONSerialization.data(withJSONObject: body)
        }

        let (data, response) = try await URLSession.shared.data(for: request)

        guard let httpResponse = response as? HTTPURLResponse, httpResponse.statusCode == 200 else {
            let statusCode = (response as? HTTPURLResponse)?.statusCode ?? 0
            let errorMsg = (try? JSONSerialization.jsonObject(with: data) as? [String: Any])?["error"] as? String
            throw AgentServiceError.commandFailed(errorMsg ?? "HTTP \(statusCode)")
        }

        guard let json = try JSONSerialization.jsonObject(with: data) as? [String: Any],
              let ok = json["ok"] as? Bool, ok else {
            let errorMsg = (try? JSONSerialization.jsonObject(with: data) as? [String: Any])?["error"] as? String
            throw AgentServiceError.commandFailed(errorMsg ?? "Invalid response")
        }
    }

    public func stopAgent(agentId: String) async throws {
        try await runShellCommand(["lfd", "stop", agentId])
    }

    /// Clone an agent with a new name.
    public func cloneAgent(agentId: String, name: String? = nil) async throws -> Agent {
        let baseURL = URL(string: "http://127.0.0.1:8765")!
        let url = baseURL.appendingPathComponent("agents/\(agentId)/clone")

        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")

        if let name = name {
            let body: [String: Any] = ["name": name]
            request.httpBody = try JSONSerialization.data(withJSONObject: body)
        }

        let (data, response) = try await URLSession.shared.data(for: request)

        guard let httpResponse = response as? HTTPURLResponse, httpResponse.statusCode == 200 else {
            let statusCode = (response as? HTTPURLResponse)?.statusCode ?? 0
            let errorMsg = (try? JSONSerialization.jsonObject(with: data) as? [String: Any])?["error"] as? String
            throw AgentServiceError.commandFailed(errorMsg ?? "HTTP \(statusCode)")
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

    private func decodeOptionalStringArray(_ str: String?) -> [String]? {
        guard let str, !str.isEmpty else { return nil }
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
