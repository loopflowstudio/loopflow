// Service for loading Wave and FlowRun data from lfd.db SQLite database.

import Foundation
import SQLite3

public struct WaveService: @unchecked Sendable {
    public init() {}
    private let dbPath = FileManager.default.homeDirectoryForCurrentUser
        .appendingPathComponent(".lf/lfd.db").path

    private var session: URLSession {
        let config = URLSessionConfiguration.default
        config.timeoutIntervalForRequest = 3  // Fast fail if daemon not running
        config.timeoutIntervalForResource = 10
        return URLSession(configuration: config)
    }

    // MARK: - Waves

    /// List waves for a repository via HTTP API.
    /// The API normalizes worktree paths to their main repo.
    public func listWaves(repo: URL) async throws -> [Wave] {
        let baseURL = URL(string: "http://127.0.0.1:8765")!
        var components = URLComponents(url: baseURL.appendingPathComponent("waves"), resolvingAgainstBaseURL: false)!
        components.queryItems = [URLQueryItem(name: "repo", value: repo.path)]

        guard let url = components.url else { return [] }

        do {
            let (data, response) = try await session.data(from: url)

            guard let httpResponse = response as? HTTPURLResponse, httpResponse.statusCode == 200 else {
                return []
            }

            guard let json = try JSONSerialization.jsonObject(with: data) as? [String: Any],
                  let ok = json["ok"] as? Bool, ok,
                  let result = json["result"] as? [String: Any],
                  let wavesData = result["waves"] as? [[String: Any]] else {
                return []
            }

            return wavesData.map { parseWaveFromJSON($0) }
        } catch {
            return []
        }
    }

    // MARK: - FlowRuns

    public func listFlowRuns(waveId: String? = nil, repo: URL? = nil, limit: Int = 50) async throws -> [FlowRun] {
        var flowRuns: [FlowRun] = []

        var db: OpaquePointer?
        guard sqlite3_open(dbPath, &db) == SQLITE_OK else {
            return []
        }
        defer { sqlite3_close(db) }

        var query = """
            SELECT id, wave_id, flow, area, repo, direction, status, iteration,
                   worktree, branch, current_step, error, pr_url,
                   started_at, ended_at, created_at
            FROM runs
        """

        var conditions: [String] = []
        if waveId != nil {
            conditions.append("wave_id = ?")
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
        if let waveId {
            sqlite3_bind_text(stmt, paramIndex, waveId, -1, nil)
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

            let waveId = columnText(stmt, 1)
            let area = columnText(stmt, 3) ?? "."
            let direction = decodeStringArray(columnText(stmt, 5))
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
                waveId: waveId,
                flow: flow,
                area: area,
                repo: repoPath,
                direction: direction,
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

    public func getFlowRuns(forWave waveId: String, limit: Int = 10) async throws -> [FlowRun] {
        return try await listFlowRuns(waveId: waveId, limit: limit)
    }

    // MARK: - Actions

    public func connectLfd() async throws {
        try await runShellCommand(["lfd", "install"])
    }

    public func createWave(name: String, repo: URL) async throws -> Wave {
        // Create wave via lfd HTTP API with minimal config
        // User configures area, direction, flow in detail panel before running
        let baseURL = URL(string: "http://127.0.0.1:8765")!
        var components = URLComponents(url: baseURL.appendingPathComponent("waves"), resolvingAgainstBaseURL: false)!
        components.queryItems = [URLQueryItem(name: "repo", value: repo.path)]

        var request = URLRequest(url: components.url!)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")

        // Create with defaults - lfd requires non-empty direction/area
        let body: [String: Any] = [
            "name": name.isEmpty ? NSNull() : name,
            "flow": "design",  // Default flow (single step)
            "direction": ["default"],
            "area": ["."]  // Root directory
        ]
        request.httpBody = try JSONSerialization.data(withJSONObject: body)

        let (data, response) = try await session.data(for: request)

        guard let httpResponse = response as? HTTPURLResponse, httpResponse.statusCode == 200 else {
            let statusCode = (response as? HTTPURLResponse)?.statusCode ?? 0
            throw WaveServiceError.commandFailed("HTTP \(statusCode)")
        }

        guard let json = try JSONSerialization.jsonObject(with: data) as? [String: Any],
              let ok = json["ok"] as? Bool, ok,
              let result = json["result"] as? [String: Any],
              let waveData = result["wave"] as? [String: Any] else {
            let errorMsg = (try? JSONSerialization.jsonObject(with: data) as? [String: Any])?["error"] as? String
            throw WaveServiceError.commandFailed(errorMsg ?? "Invalid response")
        }

        return parseWaveFromJSON(waveData)
    }

    private func parseWaveFromJSON(_ json: [String: Any]) -> Wave {
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

        // Parse PR URL
        var prURL: URL?
        if let urlStr = json["pr_url"] as? String {
            prURL = URL(string: urlStr)
        }

        // Parse PR state
        var prState: PRState?
        if let stateStr = json["pr_state"] as? String {
            prState = PRState(rawValue: stateStr.lowercased())
        }

        // Parse staleness
        var staleness: Staleness = .active
        if let stalenessStr = json["staleness"] as? String {
            switch stalenessStr {
            case "merged": staleness = .merged
            case "remote_deleted": staleness = .remoteDeleted
            default:
                if let days = json["staleness_days"] as? Int {
                    staleness = .inactive(days: days)
                }
            }
        }

        // Parse recent steps
        var recentSteps: [StepRun] = []
        if let stepsData = json["recent_steps"] as? [[String: Any]] {
            recentSteps = stepsData.compactMap { stepJson -> StepRun? in
                guard let id = stepJson["id"] as? String,
                      let step = stepJson["step"] as? String,
                      let status = stepJson["status"] as? String else {
                    return nil
                }
                let stepRunJSON = StepRunJSON(
                    id: id,
                    step: step,
                    status: status,
                    startedAt: stepJson["started_at"] as? String,
                    endedAt: stepJson["ended_at"] as? String
                )
                return StepRun(from: stepRunJSON)
            }
        }

        return Wave(
            id: json["id"] as? String ?? UUID().uuidString,
            name: json["name"] as? String ?? "",
            area: json["area"] as? [String],
            direction: json["direction"] as? [String],
            flow: json["flow"] as? String ?? "design",
            repo: json["repo"] as? String ?? "",
            stimulus: stimulus,
            paused: json["paused"] as? Bool ?? true,
            status: WaveStatus(rawValue: json["status"] as? String ?? "idle") ?? .idle,
            iteration: json["iteration"] as? Int ?? 0,
            worktreePath: json["worktree"] as? String,
            branch: json["branch"] as? String,
            isDirty: json["is_dirty"] as? Bool ?? false,
            isRebasing: json["is_rebasing"] as? Bool ?? false,
            isMerging: json["is_merging"] as? Bool ?? false,
            hasDiff: json["has_diff"] as? Bool ?? false,
            aheadMain: json["ahead_main"] as? Int ?? 0,
            behindMain: json["behind_main"] as? Int ?? 0,
            aheadRemote: json["ahead_remote"] as? Int ?? 0,
            behindRemote: json["behind_remote"] as? Int ?? 0,
            prURL: prURL,
            prNumber: json["pr_number"] as? Int,
            prState: prState,
            staleness: staleness,
            recentSteps: recentSteps,
            prLimit: json["pr_limit"] as? Int ?? 5,
            mergeMode: MergeMode(rawValue: json["merge_mode"] as? String ?? "pr") ?? .pr,
            pid: json["pid"] as? Int,
            createdAt: createdAt
        )
    }

    /// Update wave configuration (name, area, direction, flow, stimulus, paused).
    public func updateWave(
        waveId: String,
        name: String? = nil,
        area: [String]? = nil,
        direction: [String]? = nil,
        flow: String? = nil,
        stimulus: Stimulus? = nil,
        paused: Bool? = nil
    ) async throws -> Wave {
        let baseURL = URL(string: "http://127.0.0.1:8765")!
        let url = baseURL.appendingPathComponent("waves/\(waveId)")

        var request = URLRequest(url: url)
        request.httpMethod = "PATCH"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")

        var body: [String: Any] = [:]
        if let name = name { body["name"] = name }
        if let area = area { body["area"] = area }
        if let direction = direction { body["direction"] = direction }
        if let flow = flow { body["flow"] = flow }
        if let stimulus = stimulus {
            var stimDict: [String: Any] = ["kind": stimulus.kind.rawValue]
            if let cron = stimulus.cron { stimDict["cron"] = cron }
            body["stimulus"] = stimDict
        }
        if let paused = paused { body["paused"] = paused }

        request.httpBody = try JSONSerialization.data(withJSONObject: body)

        let (data, response) = try await session.data(for: request)

        guard let httpResponse = response as? HTTPURLResponse, httpResponse.statusCode == 200 else {
            let statusCode = (response as? HTTPURLResponse)?.statusCode ?? 0
            let errorMsg = (try? JSONSerialization.jsonObject(with: data) as? [String: Any])?["error"] as? String
            throw WaveServiceError.commandFailed(errorMsg ?? "HTTP \(statusCode)")
        }

        guard let json = try JSONSerialization.jsonObject(with: data) as? [String: Any],
              let ok = json["ok"] as? Bool, ok,
              let result = json["result"] as? [String: Any],
              let waveData = result["wave"] as? [String: Any] else {
            let errorMsg = (try? JSONSerialization.jsonObject(with: data) as? [String: Any])?["error"] as? String
            throw WaveServiceError.commandFailed(errorMsg ?? "Invalid response")
        }

        return parseWaveFromJSON(waveData)
    }

    /// Run wave, optionally with one-time overrides.
    public func run(
        waveId: String,
        area: [String]? = nil,
        direction: [String]? = nil,
        flow: String? = nil,
        stimulus: Stimulus? = nil
    ) async throws {
        let baseURL = URL(string: "http://127.0.0.1:8765")!
        let url = baseURL.appendingPathComponent("waves/\(waveId)/run")

        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")

        // Build body with any overrides
        var body: [String: Any] = [:]
        if let area = area { body["area"] = area }
        if let direction = direction { body["direction"] = direction }
        if let flow = flow { body["flow"] = flow }
        if let stimulus = stimulus {
            var stimDict: [String: Any] = ["kind": stimulus.kind.rawValue]
            if let cron = stimulus.cron { stimDict["cron"] = cron }
            body["stimulus"] = stimDict
        }

        if !body.isEmpty {
            request.httpBody = try JSONSerialization.data(withJSONObject: body)
        }

        let (data, response) = try await session.data(for: request)

        guard let httpResponse = response as? HTTPURLResponse, httpResponse.statusCode == 200 else {
            let statusCode = (response as? HTTPURLResponse)?.statusCode ?? 0
            let errorMsg = (try? JSONSerialization.jsonObject(with: data) as? [String: Any])?["error"] as? String
            throw WaveServiceError.commandFailed(errorMsg ?? "HTTP \(statusCode)")
        }

        guard let json = try JSONSerialization.jsonObject(with: data) as? [String: Any],
              let ok = json["ok"] as? Bool, ok else {
            let errorMsg = (try? JSONSerialization.jsonObject(with: data) as? [String: Any])?["error"] as? String
            throw WaveServiceError.commandFailed(errorMsg ?? "Invalid response")
        }
    }

    public func stopWave(waveId: String) async throws {
        try await runShellCommand(["lfd", "stop", waveId])
    }

    /// Clone a wave with a new name.
    public func cloneWave(waveId: String, name: String? = nil) async throws -> Wave {
        let baseURL = URL(string: "http://127.0.0.1:8765")!
        let url = baseURL.appendingPathComponent("waves/\(waveId)/clone")

        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")

        if let name = name {
            let body: [String: Any] = ["name": name]
            request.httpBody = try JSONSerialization.data(withJSONObject: body)
        }

        let (data, response) = try await session.data(for: request)

        guard let httpResponse = response as? HTTPURLResponse, httpResponse.statusCode == 200 else {
            let statusCode = (response as? HTTPURLResponse)?.statusCode ?? 0
            let errorMsg = (try? JSONSerialization.jsonObject(with: data) as? [String: Any])?["error"] as? String
            throw WaveServiceError.commandFailed(errorMsg ?? "HTTP \(statusCode)")
        }

        guard let json = try JSONSerialization.jsonObject(with: data) as? [String: Any],
              let ok = json["ok"] as? Bool, ok,
              let result = json["result"] as? [String: Any],
              let waveData = result["wave"] as? [String: Any] else {
            let errorMsg = (try? JSONSerialization.jsonObject(with: data) as? [String: Any])?["error"] as? String
            throw WaveServiceError.commandFailed(errorMsg ?? "Invalid response")
        }

        return parseWaveFromJSON(waveData)
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
                    continuation.resume(throwing: WaveServiceError.commandFailed(output.isEmpty ? "Exit code \(process.terminationStatus)" : output))
                }
            } catch {
                continuation.resume(throwing: error)
            }
        }
    }
}

public enum WaveServiceError: LocalizedError {
    case commandFailed(String)

    public var errorDescription: String? {
        switch self {
        case .commandFailed(let message):
            return message
        }
    }
}
