// Service for loading Wave and WaveRun data from lfd daemon (HTTP).

import Foundation

public struct LocalWaveService: WaveServiceProtocol, @unchecked Sendable {
    public init() {}
    private let baseURL = lfdBaseURL
    private let apiBaseURL = lfdApiBaseURL

    private var session: URLSession {
        let config = URLSessionConfiguration.default
        config.timeoutIntervalForRequest = 3  // Fast fail if daemon not running
        config.timeoutIntervalForResource = 10
        return URLSession(configuration: config)
    }

    /// Session with longer timeouts for operations that involve git (fetch, push, worktree).
    private var longSession: URLSession {
        let config = URLSessionConfiguration.default
        config.timeoutIntervalForRequest = 30  // Git operations can be slow
        config.timeoutIntervalForResource = 60
        return URLSession(configuration: config)
    }

    // MARK: - Waves

    /// List waves for a repository via HTTP API.
    /// The API normalizes worktree paths to their main repo.
    public func listWaves(repo: URL) async throws -> [Wave] {
        var components = URLComponents(url: apiBaseURL.appendingPathComponent("waves"), resolvingAgainstBaseURL: false)!
        components.queryItems = [URLQueryItem(name: "repo", value: repo.path)]

        guard let url = components.url else {
            LoggingService.lfd("listWaves: invalid URL for repo=\(repo.path)")
            return []
        }

        LoggingService.lfd("listWaves: GET \(url)")

        do {
            // Use longSession - list may include worktree state enrichment which runs git
            let (data, response) = try await longSession.data(from: url)

            guard let httpResponse = response as? HTTPURLResponse else {
                LoggingService.lfd("listWaves: no HTTP response")
                return []
            }

            LoggingService.lfd("listWaves: status=\(httpResponse.statusCode)")

            guard httpResponse.statusCode == 200 else {
                LoggingService.lfd("listWaves: non-200 status")
                return []
            }

            guard let json = try JSONSerialization.jsonObject(with: data) as? [String: Any],
                  let wavesData = json["data"] as? [[String: Any]] else {
                LoggingService.lfd("listWaves: invalid JSON response")
                return []
            }

            let waves = wavesData.map { parseWaveFromJSON($0) }
            LoggingService.lfd("listWaves: found \(waves.count) waves")
            return waves
        } catch {
            LoggingService.lfd("listWaves: error=\(error.localizedDescription)")
            return []
        }
    }

    // MARK: - WaveRuns

    public func listWaveRuns(waveId: String? = nil, repo: URL? = nil, limit: Int = 50) async throws -> [WaveRun] {
        var components = URLComponents(url: apiBaseURL.appendingPathComponent("wave_runs"), resolvingAgainstBaseURL: false)!
        var queryItems: [URLQueryItem] = [URLQueryItem(name: "limit", value: String(limit))]
        if let waveId {
            queryItems.append(URLQueryItem(name: "wave_id", value: waveId))
        }
        if let repo {
            queryItems.append(URLQueryItem(name: "repo", value: repo.path))
        }
        components.queryItems = queryItems

        guard let url = components.url else {
            LoggingService.lfd("listWaveRuns: invalid URL")
            return []
        }

        LoggingService.lfd("listWaveRuns: GET \(url)")

        do {
            let (data, response) = try await session.data(from: url)

            guard let httpResponse = response as? HTTPURLResponse else {
                LoggingService.lfd("listWaveRuns: no HTTP response")
                return []
            }

            guard httpResponse.statusCode == 200 else {
                LoggingService.lfd("listWaveRuns: non-200 status")
                return []
            }

            guard let json = try JSONSerialization.jsonObject(with: data) as? [String: Any],
                  let runsData = json["data"] as? [[String: Any]] else {
                LoggingService.lfd("listWaveRuns: invalid JSON response")
                return []
            }

            return runsData.compactMap { parseWaveRunFromJSON($0) }
        } catch {
            LoggingService.lfd("listWaveRuns: error=\(error.localizedDescription)")
            return []
        }
    }

    // MARK: - Actions

    public func connectLfd() async throws {
        LoggingService.lfd("connectLfd: running 'lfd install'")
        try await runShellCommand(["lfd", "install"])
        LoggingService.lfd("connectLfd: 'lfd install' completed")
    }

    public func checkAvailability() async -> Bool {
        let url = baseURL.appendingPathComponent("status")
        do {
            let (_, response) = try await session.data(from: url)
            return (response as? HTTPURLResponse)?.statusCode == 200
        } catch {
            return false
        }
    }

    public func createWave(name: String, repo: URL) async throws -> Wave {
        LoggingService.lfd("createWave: name=\(name.isEmpty ? "(auto)" : name) repo=\(repo.path)")

        // Create wave via lfd HTTP API with minimal config
        // User configures area, direction, flow in detail panel before running
        var request = URLRequest(url: apiBaseURL.appendingPathComponent("waves"))
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")

        // Create with defaults - lfd requires non-empty direction/area
        let body: [String: Any] = [
            "repo": repo.path,
            "name": name.isEmpty ? NSNull() : name,
            "flow": "design",  // Default flow (single step)
            "direction": ["default"],
            "area": ["."]  // Root directory
        ]
        request.httpBody = try JSONSerialization.data(withJSONObject: body)

        LoggingService.lfd("createWave: POST \(request.url!) body=\(body)")

        do {
            // Use longSession - createWave does git fetch + worktree add + push
            let (data, response) = try await longSession.data(for: request)

            guard let httpResponse = response as? HTTPURLResponse else {
                LoggingService.lfd("createWave: no HTTP response")
                throw WaveServiceError.commandFailed("No response from lfd")
            }

            LoggingService.lfd("createWave: status=\(httpResponse.statusCode)")

            guard httpResponse.statusCode == 200 else {
                let errorBody = String(data: data, encoding: .utf8) ?? ""
                LoggingService.lfd("createWave: error response=\(errorBody)")
                throw WaveServiceError.commandFailed("HTTP \(httpResponse.statusCode)")
            }

            guard let json = try JSONSerialization.jsonObject(with: data) as? [String: Any] else {
                let errorMsg = parseErrorMessage(data) ?? "Invalid response"
                LoggingService.lfd("createWave: invalid response error=\(errorMsg)")
                throw WaveServiceError.commandFailed(errorMsg)
            }

            let wave = parseWaveFromJSON(json)
            LoggingService.lfd("createWave: success id=\(wave.id) name=\(wave.name)")
            return wave
        } catch let error as WaveServiceError {
            throw error
        } catch {
            LoggingService.lfd("createWave: exception=\(error.localizedDescription)")
            throw WaveServiceError.commandFailed(error.localizedDescription)
        }
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

        // Parse waiting reason
        var waitingReason: WaitingReason?
        if let reason = json["waiting_reason"] as? String,
           reason == "pr_limit_reached",
           let openPRs = json["open_prs"] as? Int {
            let prLimit = json["pr_limit"] as? Int ?? 5
            waitingReason = .prLimitReached(open: openPRs, limit: prLimit)
        }

        // Parse flow progress fields
        let stepIndex = json["step_index"] as? Int ?? 0
        let flowSteps = json["flow_steps"] as? [String]
        var runStartedAt: Date?
        if let dateStr = json["run_started_at"] as? String {
            runStartedAt = dateFormatter.date(from: dateStr)
        }

        let status = WaveStatus(rawValue: json["status"] as? String ?? "idle") ?? .idle
        let paused = status == .paused

        return Wave(
            id: json["id"] as? String ?? UUID().uuidString,
            name: json["name"] as? String ?? "",
            area: json["area"] as? [String],
            direction: json["direction"] as? [String],
            flow: json["flow"] as? String ?? "design",
            repo: json["repo"] as? String ?? "",
            stimulus: stimulus,
            paused: paused,
            status: status,
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
            createdAt: createdAt,
            waitingReason: waitingReason,
            stepIndex: stepIndex,
            flowSteps: flowSteps,
            runStartedAt: runStartedAt
        )
    }

    /// Update wave configuration (name, area, direction, flow, stimulus, paused).
    public func updateWave(_ id: String, config: WaveConfigUpdate) async throws -> Wave {
        let url = apiBaseURL.appendingPathComponent("waves/\(id)")

        var request = URLRequest(url: url)
        request.httpMethod = "PATCH"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")

        var body: [String: Any] = [:]
        if let name = config.name { body["name"] = name }
        if let area = config.area { body["area"] = area }
        if let direction = config.direction { body["direction"] = direction }
        if let flow = config.flow { body["flow"] = flow }
        if let stimulus = config.stimulus {
            var stimDict: [String: Any] = ["kind": stimulus.kind.rawValue]
            if let cron = stimulus.cron { stimDict["cron"] = cron }
            body["stimulus"] = stimDict
        }
        if let paused = config.paused { body["paused"] = paused }

        request.httpBody = try JSONSerialization.data(withJSONObject: body)

        let (data, response) = try await session.data(for: request)

        guard let httpResponse = response as? HTTPURLResponse, httpResponse.statusCode == 200 else {
            let statusCode = (response as? HTTPURLResponse)?.statusCode ?? 0
            let errorMsg = parseErrorMessage(data)
            throw WaveServiceError.commandFailed(errorMsg ?? "HTTP \(statusCode)")
        }

        guard let json = try JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            let errorMsg = parseErrorMessage(data)
            throw WaveServiceError.commandFailed(errorMsg ?? "Invalid response")
        }

        return parseWaveFromJSON(json)
    }

    /// Run wave, optionally with one-time overrides.
    public func run(_ id: String, overrides: RunOverrides?) async throws {
        let url = apiBaseURL.appendingPathComponent("waves/\(id)/run")

        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")

        // Build body with any overrides
        var body: [String: Any] = [:]
        if let overrides {
            if let area = overrides.area { body["area"] = area }
            if let direction = overrides.direction { body["direction"] = direction }
            if let flow = overrides.flow { body["flow"] = flow }
            if let stimulus = overrides.stimulus {
                var stimDict: [String: Any] = ["kind": stimulus.kind.rawValue]
                if let cron = stimulus.cron { stimDict["cron"] = cron }
                body["stimulus"] = stimDict
            }
        }

        if !body.isEmpty {
            request.httpBody = try JSONSerialization.data(withJSONObject: body)
        }

        let (data, response) = try await session.data(for: request)

        guard let httpResponse = response as? HTTPURLResponse, httpResponse.statusCode == 200 else {
            let statusCode = (response as? HTTPURLResponse)?.statusCode ?? 0
            let errorMsg = parseErrorMessage(data)
            throw WaveServiceError.commandFailed(errorMsg ?? "HTTP \(statusCode)")
        }

        guard (try? JSONSerialization.jsonObject(with: data)) != nil else {
            let errorMsg = parseErrorMessage(data)
            throw WaveServiceError.commandFailed(errorMsg ?? "Invalid response")
        }
    }

    public func connect(_ id: String) async throws -> ConnectionInfo {
        let url = apiBaseURL.appendingPathComponent("waves/\(id)/connect")

        var request = URLRequest(url: url)
        request.httpMethod = "POST"

        let (data, response) = try await session.data(for: request)

        guard let httpResponse = response as? HTTPURLResponse, httpResponse.statusCode == 200 else {
            let statusCode = (response as? HTTPURLResponse)?.statusCode ?? 0
            let errorMsg = (try? JSONSerialization.jsonObject(with: data) as? [String: Any])?["detail"] as? String
            throw WaveServiceError.commandFailed(errorMsg ?? "HTTP \(statusCode)")
        }

        guard let json = try JSONSerialization.jsonObject(with: data) as? [String: Any],
              let worktree = json["worktree"] as? String,
              let step = json["step"] as? String,
              let agentId = json["agent_id"] as? String,
              let promptFile = json["prompt_file"] as? String else {
            throw WaveServiceError.commandFailed("Invalid response")
        }

        let waveRunId = json["wave_run_id"] as? String
        let stepIndex = json["step_index"] as? Int ?? 0

        return ConnectionInfo(
            worktree: worktree,
            step: step,
            agentId: agentId,
            promptFile: promptFile,
            waveRunId: waveRunId,
            stepIndex: stepIndex
        )
    }

    public func stop(_ id: String) async throws {
        try await runShellCommand(["lfd", "stop", id])
    }

    /// Clone a wave with a new name.
    public func cloneWave(_ id: String, name: String?) async throws -> Wave {
        let url = apiBaseURL.appendingPathComponent("waves/\(id)/clone")

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
            let errorMsg = parseErrorMessage(data)
            throw WaveServiceError.commandFailed(errorMsg ?? "HTTP \(statusCode)")
        }

        guard let json = try JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            let errorMsg = parseErrorMessage(data)
            throw WaveServiceError.commandFailed(errorMsg ?? "Invalid response")
        }

        return parseWaveFromJSON(json)
    }

    /// Delete a wave.
    public func deleteWave(_ id: String) async throws {
        let url = apiBaseURL.appendingPathComponent("waves/\(id)")

        var request = URLRequest(url: url)
        request.httpMethod = "DELETE"

        let (data, response) = try await session.data(for: request)

        guard let httpResponse = response as? HTTPURLResponse, httpResponse.statusCode == 200 else {
            let statusCode = (response as? HTTPURLResponse)?.statusCode ?? 0
            let errorMsg = parseErrorMessage(data)
            throw WaveServiceError.commandFailed(errorMsg ?? "HTTP \(statusCode)")
        }

        guard (try? JSONSerialization.jsonObject(with: data)) != nil else {
            let errorMsg = parseErrorMessage(data)
            throw WaveServiceError.commandFailed(errorMsg ?? "Invalid response")
        }
    }

    /// Collapse all outstanding PRs for a wave into a single PR.
    /// Returns the new PR URL on success.
    public func collapsePRs(_ id: String) async throws -> CollapsePRsResult {
        let url = apiBaseURL.appendingPathComponent("waves/\(id)/collapse")

        var request = URLRequest(url: url)
        request.httpMethod = "POST"

        // Use longSession - collapse does multiple git/gh operations
        let (data, response) = try await longSession.data(for: request)

        guard let httpResponse = response as? HTTPURLResponse, httpResponse.statusCode == 200 else {
            let statusCode = (response as? HTTPURLResponse)?.statusCode ?? 0
            let errorMsg = parseErrorMessage(data)
            throw WaveServiceError.commandFailed(errorMsg ?? "HTTP \(statusCode)")
        }

        guard let json = try JSONSerialization.jsonObject(with: data) as? [String: Any],
              let ok = json["ok"] as? Bool else {
            throw WaveServiceError.commandFailed("Invalid response")
        }

        if ok, let result = json["result"] as? [String: Any] {
            let newPRUrl = result["new_pr_url"] as? String
            let closedPRs = result["closed_prs"] as? [Int] ?? []
            return CollapsePRsResult(newPRUrl: newPRUrl, closedPRs: closedPRs)
        } else {
            let errorMsg = json["error"] as? String ?? "Collapse failed"
            throw WaveServiceError.commandFailed(errorMsg)
        }
    }

    // MARK: - Private helpers

    private func parseWaveRunFromJSON(_ json: [String: Any]) -> WaveRun? {
        guard let id = json["id"] as? String,
              let flow = json["flow"] as? String,
              let repoPath = json["repo"] as? String else {
            return nil
        }

        let waveId = json["wave_id"] as? String
        let area = normalizeArea(json["area"])
        let direction = normalizeDirection(json["direction"])
        let statusStr = json["status"] as? String ?? WaveRunStatus.pending.rawValue
        let status = WaveRunStatus(rawValue: statusStr) ?? .pending
        let iteration = normalizeInt(json["iteration"])

        let pr: PullRequest?
        if let prData = json["pr"] as? [String: Any],
           let urlString = prData["url"] as? String,
           let url = URL(string: urlString) {
            let number = prData["number"] as? Int
            let state = (prData["state"] as? String).map { $0.lowercased() }.flatMap { PRState(rawValue: $0) }
            let title = prData["title"] as? String
            let branch = prData["branch"] as? String
            pr = PullRequest(url: url, number: number, state: state, title: title, branch: branch)
        } else {
            pr = nil
        }

        return WaveRun(
            id: id,
            waveId: waveId,
            flow: flow,
            area: area,
            repo: repoPath,
            direction: direction,
            status: status,
            iteration: iteration,
            worktree: json["local_worktree"] as? String,
            branch: json["remote_branch"] as? String,
            currentStep: json["current_step"] as? String,
            error: json["error"] as? String,
            pr: pr,
            startedAt: parseDate(json["started_at"]),
            endedAt: parseDate(json["ended_at"]),
            createdAt: parseDate(json["created_at"]) ?? Date()
        )
    }

    private func normalizeArea(_ value: Any?) -> String {
        if let area = value as? String {
            let decoded = decodeStringArray(area)
            if decoded.count == 1 { return decoded[0] }
            if !decoded.isEmpty { return decoded.joined(separator: ", ") }
            return area
        }
        if let areas = value as? [String] {
            if areas.count == 1 { return areas[0] }
            if !areas.isEmpty { return areas.joined(separator: ", ") }
        }
        return "."
    }

    private func normalizeDirection(_ value: Any?) -> [String] {
        if let list = value as? [String] { return list }
        if let str = value as? String { return decodeStringArray(str) }
        return []
    }

    private func normalizeInt(_ value: Any?) -> Int {
        if let intValue = value as? Int { return intValue }
        if let doubleValue = value as? Double { return Int(doubleValue) }
        return 0
    }

    private func parseDate(_ value: Any?) -> Date? {
        guard let dateStr = value as? String else { return nil }
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        if let date = formatter.date(from: dateStr) {
            return date
        }
        return ISO8601DateFormatter().date(from: dateStr)
    }

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

    private func parseErrorMessage(_ data: Data) -> String? {
        guard let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            return nil
        }
        if let error = json["error"] as? String {
            return error
        }
        if let error = json["error"] as? [String: Any],
           let message = error["message"] as? String {
            return message
        }
        return nil
    }

    private func runShellCommand(_ args: [String]) async throws {
        let cmd = args.joined(separator: " ")
        LoggingService.lfd("runShellCommand: \(cmd)")

        try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Void, Error>) in
            let process = Process()
            process.executableURL = URL(fileURLWithPath: "/bin/zsh")
            process.arguments = ["-l", "-c", cmd]

            let pipe = Pipe()
            process.standardOutput = pipe
            process.standardError = pipe

            do {
                try process.run()
                process.waitUntilExit()

                let data = pipe.fileHandleForReading.readDataToEndOfFile()
                let output = String(data: data, encoding: .utf8)?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""

                LoggingService.lfd("runShellCommand: exit=\(process.terminationStatus) output=\(output.prefix(200))")

                if process.terminationStatus == 0 {
                    continuation.resume()
                } else {
                    continuation.resume(throwing: WaveServiceError.commandFailed(output.isEmpty ? "Exit code \(process.terminationStatus)" : output))
                }
            } catch {
                LoggingService.lfd("runShellCommand: exception=\(error.localizedDescription)")
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

public struct CollapsePRsResult: Sendable {
    public let newPRUrl: String?
    public let closedPRs: [Int]

    public init(newPRUrl: String?, closedPRs: [Int]) {
        self.newPRUrl = newPRUrl
        self.closedPRs = closedPRs
    }
}
