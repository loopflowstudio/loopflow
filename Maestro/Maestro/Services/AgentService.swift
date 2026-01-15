// Service for loading agents from markdown files and runtime state from SQLite.

import Foundation
import SQLite3

struct AgentService {
    private let agentsDir = FileManager.default.homeDirectoryForCurrentUser
        .appendingPathComponent(".lf/agents")
    private let dbPath = FileManager.default.homeDirectoryForCurrentUser
        .appendingPathComponent(".lf/lfd.db").path

    func list() async throws -> [Agent] {
        var agents: [Agent] = []

        // List all .md files in agents directory
        let fm = FileManager.default
        guard fm.fileExists(atPath: agentsDir.path) else {
            return []
        }

        let files = try fm.contentsOfDirectory(at: agentsDir, includingPropertiesForKeys: nil)
            .filter { $0.pathExtension == "md" }

        for file in files {
            if let agent = parseAgentFile(file) {
                agents.append(agent)
            }
        }

        // Load runtime state from DB
        let runtimeState = loadRuntimeState()
        for i in 0..<agents.count {
            if let state = runtimeState[agents[i].name] {
                agents[i].status = state.status
                agents[i].lastRunAt = state.lastRunAt
                agents[i].iteration = state.iteration
                agents[i].pid = state.pid
                agents[i].worktree = state.worktree
            }
        }

        return agents.sorted { $0.name < $1.name }
    }

    func start(agentId: String, repoRoot: URL) async throws {
        try await runLfCommand(["agent", "start", agentId])
    }

    func stop(agentId: String) async throws {
        try await runLfCommand(["agent", "stop", agentId])
    }

    func create(name: String, emoji: String, repo: URL) throws {
        let fm = FileManager.default
        try fm.createDirectory(at: agentsDir, withIntermediateDirectories: true)

        let path = agentsDir.appendingPathComponent("\(name).md")
        let content = """
        ---
        emoji: \(emoji)
        repo: \(repo.path)
        pipeline: ship
        trigger: manual
        merge: pr
        ---

        Describe what this agent should do.
        """
        try content.write(to: path, atomically: true, encoding: .utf8)
    }

    func save(_ agent: Agent) throws {
        let path = agentsDir.appendingPathComponent("\(agent.id).md")

        var lines = ["---"]
        if !agent.emoji.isEmpty {
            lines.append("emoji: \(agent.emoji)")
        }
        lines.append("repo: \(agent.repo)")
        lines.append("pipeline: \(agent.pipeline)")
        lines.append("trigger: \(agent.triggerKind.rawValue)")
        if agent.triggerKind == .cron, let cron = agent.cron {
            lines.append("cron: \(cron)")
        }
        lines.append("merge: \(agent.mergeStrategy.rawValue)")
        if !agent.context.isEmpty {
            lines.append("context: [\(agent.context.joined(separator: ", "))]")
        }
        lines.append("---")
        lines.append("")
        lines.append(agent.prompt)

        let content = lines.joined(separator: "\n")
        try content.write(to: path, atomically: true, encoding: .utf8)
    }

    func delete(_ agent: Agent) throws {
        let path = agentsDir.appendingPathComponent("\(agent.id).md")
        try FileManager.default.removeItem(at: path)
    }

    private func parseAgentFile(_ url: URL) -> Agent? {
        guard let content = try? String(contentsOf: url, encoding: .utf8) else {
            return nil
        }

        // Parse frontmatter
        let pattern = #"^---\s*\n(.*?)\n---\s*\n?"#
        guard let regex = try? NSRegularExpression(pattern: pattern, options: .dotMatchesLineSeparators),
              let match = regex.firstMatch(in: content, range: NSRange(content.startIndex..., in: content)),
              let frontmatterRange = Range(match.range(at: 1), in: content) else {
            return nil
        }

        let frontmatter = String(content[frontmatterRange])

        // Get prompt (body after frontmatter)
        let endIndex = content.index(frontmatterRange.upperBound, offsetBy: 3, limitedBy: content.endIndex) ?? content.endIndex
        let prompt: String
        if endIndex < content.endIndex {
            prompt = String(content[content.index(after: endIndex)...]).trimmingCharacters(in: .whitespacesAndNewlines)
        } else {
            prompt = ""
        }

        let config = parseYamlFrontmatter(frontmatter)

        guard let repo = config["repo"] as? String else {
            return nil
        }

        // Pipeline can be string or array
        let pipeline: String
        if let pipelineStr = config["pipeline"] as? String {
            pipeline = pipelineStr
        } else if let pipelineArr = config["pipeline"] as? [String] {
            pipeline = pipelineArr.joined(separator: ",")
        } else {
            pipeline = "ship"
        }

        let name = url.deletingPathExtension().lastPathComponent

        // Parse trigger
        let triggerStr = config["trigger"] as? String ?? "manual"
        let triggerKind = TriggerKind(rawValue: triggerStr) ?? .manual

        // Parse merge strategy
        let mergeStr = config["merge"] as? String ?? "pr"
        let mergeStrategy = MergeStrategy(rawValue: mergeStr) ?? .pr

        return Agent(
            id: name,
            name: name,
            repo: repo,
            pipeline: pipeline,
            triggerKind: triggerKind,
            context: config["context"] as? [String] ?? [],
            prompt: prompt,
            emoji: config["emoji"] as? String ?? "",
            cron: config["cron"] as? String,
            mergeStrategy: mergeStrategy
        )
    }

    private func parseYamlFrontmatter(_ text: String) -> [String: Any] {
        var result: [String: Any] = [:]

        for line in text.split(separator: "\n") {
            let trimmed = line.trimmingCharacters(in: .whitespaces)
            guard !trimmed.isEmpty, !trimmed.hasPrefix("#") else { continue }

            if let colonIndex = trimmed.firstIndex(of: ":") {
                let key = String(trimmed[..<colonIndex]).trimmingCharacters(in: .whitespaces)
                let value = String(trimmed[trimmed.index(after: colonIndex)...]).trimmingCharacters(in: .whitespaces)

                // Inline list: [a, b, c]
                if value.hasPrefix("[") && value.hasSuffix("]") {
                    let items = value.dropFirst().dropLast()
                        .split(separator: ",")
                        .map { String($0).trimmingCharacters(in: .whitespaces) }
                    result[key] = items
                } else if let intValue = Int(value) {
                    result[key] = intValue
                } else if !value.isEmpty {
                    result[key] = value
                }
            }
        }

        return result
    }

    private struct RuntimeState {
        var status: AgentStatus
        var lastRunAt: Date?
        var iteration: Int
        var pid: Int?
        var worktree: String?
    }

    private func loadRuntimeState() -> [String: RuntimeState] {
        var states: [String: RuntimeState] = [:]

        var db: OpaquePointer?
        guard sqlite3_open(dbPath, &db) == SQLITE_OK else {
            return states
        }
        defer { sqlite3_close(db) }

        // Query agent_runs table for latest run per agent
        let query = """
            SELECT agent_name, status, started_at, pid, worktree,
                   (SELECT COUNT(*) FROM agent_runs r2 WHERE r2.agent_name = r1.agent_name) as iteration
            FROM agent_runs r1
            WHERE started_at = (SELECT MAX(started_at) FROM agent_runs r2 WHERE r2.agent_name = r1.agent_name)
        """

        var stmt: OpaquePointer?
        guard sqlite3_prepare_v2(db, query, -1, &stmt, nil) == SQLITE_OK else {
            return states
        }
        defer { sqlite3_finalize(stmt) }

        let dateFormatter = ISO8601DateFormatter()
        dateFormatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]

        while sqlite3_step(stmt) == SQLITE_ROW {
            guard let namePtr = sqlite3_column_text(stmt, 0),
                  let statusPtr = sqlite3_column_text(stmt, 1) else {
                continue
            }

            let name = String(cString: namePtr)
            let statusStr = String(cString: statusPtr)

            var lastRunAt: Date?
            if let datePtr = sqlite3_column_text(stmt, 2) {
                let dateStr = String(cString: datePtr)
                lastRunAt = dateFormatter.date(from: dateStr) ?? ISO8601DateFormatter().date(from: dateStr)
            }

            let pid = sqlite3_column_type(stmt, 3) != SQLITE_NULL ? Int(sqlite3_column_int(stmt, 3)) : nil

            var worktree: String?
            if let worktreePtr = sqlite3_column_text(stmt, 4) {
                worktree = String(cString: worktreePtr)
            }

            let iteration = Int(sqlite3_column_int(stmt, 5))

            let status: AgentStatus
            switch statusStr {
            case "running": status = .running
            case "waiting": status = .waiting
            case "completed": status = .completed
            case "error": status = .error
            case "stopped": status = .stopped
            default: status = .idle
            }

            states[name] = RuntimeState(
                status: status,
                lastRunAt: lastRunAt,
                iteration: iteration,
                pid: pid,
                worktree: worktree
            )
        }

        return states
    }

    private func runLfCommand(_ args: [String]) async throws {
        try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Void, Error>) in
            let process = Process()
            process.executableURL = URL(fileURLWithPath: "/bin/zsh")
            process.arguments = ["-l", "-c", "lf " + args.joined(separator: " ")]

            do {
                try process.run()
                process.waitUntilExit()
                if process.terminationStatus == 0 {
                    continuation.resume()
                } else {
                    continuation.resume(throwing: AgentServiceError.commandFailed)
                }
            } catch {
                continuation.resume(throwing: error)
            }
        }
    }
}

enum AgentServiceError: LocalizedError {
    case requestFailed
    case startFailed
    case stopFailed
    case commandFailed

    var errorDescription: String? {
        switch self {
        case .requestFailed:
            return "Failed to fetch agents"
        case .startFailed:
            return "Failed to start agent"
        case .stopFailed:
            return "Failed to stop agent"
        case .commandFailed:
            return "Command failed"
        }
    }
}
