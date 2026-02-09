// Service for loading Wave and WaveRun data from lfd daemon (HTTP).

import Foundation

public struct WaveConfigUpdate: Sendable {
    public var name: String?
    public var area: [String]?
    public var direction: [String]?
    public var flow: String?
    public var stimulus: Stimulus?
    public var status: WaveStatus?

    public init(
        name: String? = nil,
        area: [String]? = nil,
        direction: [String]? = nil,
        flow: String? = nil,
        stimulus: Stimulus? = nil,
        status: WaveStatus? = nil
    ) {
        self.name = name
        self.area = area
        self.direction = direction
        self.flow = flow
        self.stimulus = stimulus
        self.status = status
    }
}

public struct RunOverrides: Sendable {
    public var area: [String]?
    public var direction: [String]?
    public var flow: String?
    public var stimulus: Stimulus?

    public init(
        area: [String]? = nil,
        direction: [String]? = nil,
        flow: String? = nil,
        stimulus: Stimulus? = nil
    ) {
        self.area = area
        self.direction = direction
        self.flow = flow
        self.stimulus = stimulus
    }
}

public struct ConnectionInfo: Sendable {
    public let worktree: String
    public let step: String
    public let agentId: String
    public let promptFile: String
    public let waveRunId: String?
    public let stepIndex: Int

    public init(
        worktree: String,
        step: String,
        agentId: String,
        promptFile: String,
        waveRunId: String?,
        stepIndex: Int
    ) {
        self.worktree = worktree
        self.step = step
        self.agentId = agentId
        self.promptFile = promptFile
        self.waveRunId = waveRunId
        self.stepIndex = stepIndex
    }
}

public struct LocalWaveService: @unchecked Sendable {
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
        components.queryItems = [
            URLQueryItem(name: "repo", value: repo.path),
            URLQueryItem(name: "expand[]", value: "active_run"),
        ]

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

            let waves = wavesData.map { Self.parseWaveFromJSON($0) }
            LoggingService.lfd("listWaves: found \(waves.count) waves")
            return waves
        } catch {
            LoggingService.lfd("listWaves: error=\(error.localizedDescription)")
            return []
        }
    }

    /// Fetch a single wave by id.
    public func getWave(_ id: String) async throws -> Wave {
        var components = URLComponents(url: apiBaseURL.appendingPathComponent("waves/\(id)"), resolvingAgainstBaseURL: false)!
        components.queryItems = [URLQueryItem(name: "expand[]", value: "active_run")]

        guard let url = components.url else {
            throw WaveServiceError.commandFailed("Invalid wave URL")
        }

        LoggingService.lfd("getWave: GET \(url)")

        let (data, response) = try await session.data(from: url)

        guard let httpResponse = response as? HTTPURLResponse else {
            throw WaveServiceError.commandFailed("No response from lfd")
        }

        guard httpResponse.statusCode == 200 else {
            let errorMsg = Self.parseErrorMessage(data)
            throw WaveServiceError.commandFailed(errorMsg ?? "HTTP \(httpResponse.statusCode)")
        }

        guard let json = try JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            throw WaveServiceError.commandFailed("Invalid response")
        }

        return Self.parseWaveFromJSON(json)
    }

    /// List flows and steps from lfd.
    /// Result from the /flows endpoint containing flows, steps, and directions.
    public struct FlowsResult: Sendable {
        public var flows: [Flow]
        public var directions: [String]
    }

    public func listFlowsAndDirections(repo: URL) async throws -> FlowsResult {
        var components = URLComponents(url: apiBaseURL.appendingPathComponent("flows"), resolvingAgainstBaseURL: false)!
        components.queryItems = [URLQueryItem(name: "repo", value: repo.path)]

        guard let url = components.url else {
            LoggingService.lfd("listFlows: invalid URL")
            return FlowsResult(flows: [], directions: [])
        }

        LoggingService.lfd("listFlows: GET \(url)")

        do {
            let (data, response) = try await session.data(from: url)
            guard let httpResponse = response as? HTTPURLResponse, httpResponse.statusCode == 200 else {
                return FlowsResult(flows: [], directions: [])
            }
            guard let json = try JSONSerialization.jsonObject(with: data) as? [String: Any],
                  let result = json["result"] as? [String: Any] else {
                return FlowsResult(flows: [], directions: [])
            }

            var allFlows: [Flow] = []
            if let flowsData = result["flows"] as? [[String: Any]] {
                for flowDict in flowsData {
                    guard let name = flowDict["name"] as? String else { continue }
                    let stepNames = flowDict["steps"] as? [String] ?? []
                    let steps = stepNames.map { Step(prompt: $0) }
                    allFlows.append(Flow(name: name, steps: steps, type: .flow))
                }
            }

            if let stepsData = result["steps"] as? [[String: Any]] {
                for stepDict in stepsData {
                    guard let name = stepDict["name"] as? String else { continue }
                    allFlows.append(Flow(name: name, steps: [Step(prompt: name)], type: .step))
                }
            }

            let flows = allFlows.filter { $0.type == .flow }.sorted { $0.name < $1.name }
            let steps = allFlows.filter { $0.type == .step }.sorted { $0.name < $1.name }
            let directions = (result["directions"] as? [String]) ?? []

            return FlowsResult(flows: flows + steps, directions: directions)
        } catch {
            return FlowsResult(flows: [], directions: [])
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

            return runsData.compactMap { Self.parseWaveRunFromJSON($0) }
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

        // Create with defaults - area left empty, user configures in detail panel
        let body: [String: Any] = [
            "repo": repo.path,
            "name": name.isEmpty ? NSNull() : name,
            "flow": "design",  // Default flow (single step)
            "direction": [],
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
                let errorMsg = Self.parseErrorMessage(data) ?? "Invalid response"
                LoggingService.lfd("createWave: invalid response error=\(errorMsg)")
                throw WaveServiceError.commandFailed(errorMsg)
            }

            let wave = Self.parseWaveFromJSON(json)
            LoggingService.lfd("createWave: success id=\(wave.id) name=\(wave.name)")
            return wave
        } catch let error as WaveServiceError {
            throw error
        } catch {
            LoggingService.lfd("createWave: exception=\(error.localizedDescription)")
            throw WaveServiceError.commandFailed(error.localizedDescription)
        }
    }

    static func parseWaveFromJSON(_ json: [String: Any]) -> Wave {
        let dateFormatter = ISO8601DateFormatter()
        dateFormatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]

        let stimulus: Stimulus
        if let stimDict = json["stimulus"] as? [String: Any] {
            let kind = Stimulus.Kind(rawValue: stimDict["kind"] as? String ?? "once") ?? .once
            stimulus = Stimulus(kind: kind, cron: stimDict["cron"] as? String)
        } else {
            stimulus = Stimulus(kind: .once)
        }

        let createdAt: Date?
        if let dateStr = json["created_at"] as? String {
            createdAt = dateFormatter.date(from: dateStr) ?? ISO8601DateFormatter().date(from: dateStr)
        } else {
            createdAt = nil
        }

        let statusValue = json["status"] as? String ?? "idle"
        let normalizedStatus: String
        switch statusValue {
        case "error": normalizedStatus = "failed"
        case "completed": normalizedStatus = "idle"
        default: normalizedStatus = statusValue
        }
        let status = WaveStatus(rawValue: normalizedStatus) ?? .idle
        let activeRun = (json["active_run"] as? [String: Any]).flatMap(Self.parseWaveRunFromJSON)

        let commits: [CommitEntry]
        if let commitsData = json["commits"] as? [[String: Any]] {
            commits = commitsData.compactMap { entry in
                guard let sha = entry["sha"] as? String,
                      let message = entry["message"] as? String else { return nil }
                return CommitEntry(sha: sha, message: message)
            }
        } else {
            commits = []
        }

        return Wave(
            id: json["id"] as? String ?? UUID().uuidString,
            name: json["name"] as? String ?? "",
            repo: json["repo"] as? String ?? "",
            flow: json["flow"] as? String ?? "design",
            direction: normalizeStringList(json["direction"]),
            area: normalizeStringList(json["area"]),
            stimulus: stimulus,
            status: status,
            iteration: json["iteration"] as? Int ?? 0,
            localWorktree: json["local_worktree"] as? String,
            remoteBranch: json["remote_branch"] as? String,
            commits: commits,
            diffStat: json["diff_stat"] as? String,
            activeRun: activeRun,
            createdAt: createdAt
        )
    }

    /// Update wave configuration (name, area, direction, flow, stimulus, status).
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
        if let status = config.status { body["status"] = status.rawValue }

        request.httpBody = try JSONSerialization.data(withJSONObject: body)

        let (data, response) = try await session.data(for: request)

        guard let httpResponse = response as? HTTPURLResponse, httpResponse.statusCode == 200 else {
            let statusCode = (response as? HTTPURLResponse)?.statusCode ?? 0
            let errorMsg = Self.parseErrorMessage(data)
            throw WaveServiceError.commandFailed(errorMsg ?? "HTTP \(statusCode)")
        }

        guard let json = try JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            let errorMsg = Self.parseErrorMessage(data)
            throw WaveServiceError.commandFailed(errorMsg ?? "Invalid response")
        }

        return Self.parseWaveFromJSON(json)
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
            let errorMsg = Self.parseErrorMessage(data)
            throw WaveServiceError.commandFailed(errorMsg ?? "HTTP \(statusCode)")
        }

        guard (try? JSONSerialization.jsonObject(with: data)) != nil else {
            let errorMsg = Self.parseErrorMessage(data)
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
        let url = apiBaseURL.appendingPathComponent("waves/\(id)/stop")

        var request = URLRequest(url: url)
        request.httpMethod = "POST"

        let (data, response) = try await session.data(for: request)

        guard let httpResponse = response as? HTTPURLResponse, httpResponse.statusCode == 200 else {
            let statusCode = (response as? HTTPURLResponse)?.statusCode ?? 0
            let errorMsg = Self.parseErrorMessage(data)
            throw WaveServiceError.commandFailed(errorMsg ?? "HTTP \(statusCode)")
        }
    }

    /// Land a wave's current branch (merge via PR).
    public func landWave(_ id: String) async throws {
        let url = apiBaseURL.appendingPathComponent("waves/\(id)/land")

        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.httpBody = try JSONSerialization.data(withJSONObject: ["create_pr": true])

        let (data, response) = try await longSession.data(for: request)

        guard let httpResponse = response as? HTTPURLResponse, httpResponse.statusCode == 200 else {
            let statusCode = (response as? HTTPURLResponse)?.statusCode ?? 0
            let errorMsg = Self.parseErrorMessage(data)
            throw WaveServiceError.commandFailed(errorMsg ?? "HTTP \(statusCode)")
        }
    }

    /// Start the next iteration of a wave (create new branch).
    public func nextWave(_ id: String) async throws -> String {
        let url = apiBaseURL.appendingPathComponent("waves/\(id)/next")

        var request = URLRequest(url: url)
        request.httpMethod = "POST"

        let (data, response) = try await longSession.data(for: request)

        guard let httpResponse = response as? HTTPURLResponse, httpResponse.statusCode == 200 else {
            let statusCode = (response as? HTTPURLResponse)?.statusCode ?? 0
            let errorMsg = Self.parseErrorMessage(data)
            throw WaveServiceError.commandFailed(errorMsg ?? "HTTP \(statusCode)")
        }

        guard let json = try JSONSerialization.jsonObject(with: data) as? [String: Any],
              let newBranch = json["new_branch"] as? String else {
            throw WaveServiceError.commandFailed("Invalid response")
        }

        return newBranch
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
            let errorMsg = Self.parseErrorMessage(data)
            throw WaveServiceError.commandFailed(errorMsg ?? "HTTP \(statusCode)")
        }

        guard let json = try JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            let errorMsg = Self.parseErrorMessage(data)
            throw WaveServiceError.commandFailed(errorMsg ?? "Invalid response")
        }

        return Self.parseWaveFromJSON(json)
    }

    /// Delete a wave.
    public func deleteWave(_ id: String) async throws {
        let url = apiBaseURL.appendingPathComponent("waves/\(id)")

        var request = URLRequest(url: url)
        request.httpMethod = "DELETE"

        let (data, response) = try await session.data(for: request)

        guard let httpResponse = response as? HTTPURLResponse, httpResponse.statusCode == 200 else {
            let statusCode = (response as? HTTPURLResponse)?.statusCode ?? 0
            let errorMsg = Self.parseErrorMessage(data)
            throw WaveServiceError.commandFailed(errorMsg ?? "HTTP \(statusCode)")
        }

        guard (try? JSONSerialization.jsonObject(with: data)) != nil else {
            let errorMsg = Self.parseErrorMessage(data)
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
            let errorMsg = Self.parseErrorMessage(data)
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

    // MARK: - Output Streaming

    /// Stream output for a wave (replay from disk + live follow).
    /// Yields one line at a time. The connection stays open for live updates.
    public func streamOutput(waveId: String) -> AsyncThrowingStream<String, Error> {
        let url = apiBaseURL.appendingPathComponent("waves/\(waveId)/logs")

        return AsyncThrowingStream { continuation in
            let task = Task {
                do {
                    let config = URLSessionConfiguration.default
                    config.timeoutIntervalForRequest = 300
                    config.timeoutIntervalForResource = 86400 // Keep alive for a day
                    let streamSession = URLSession(configuration: config)

                    let (bytes, response) = try await streamSession.bytes(from: url)

                    guard let httpResponse = response as? HTTPURLResponse,
                          httpResponse.statusCode == 200 else {
                        continuation.finish()
                        return
                    }

                    for try await line in bytes.lines {
                        continuation.yield(line)
                    }
                    continuation.finish()
                } catch {
                    continuation.finish(throwing: error)
                }
            }

            continuation.onTermination = { _ in
                task.cancel()
            }
        }
    }

    // MARK: - Private helpers

    private static func parseWaveRunFromJSON(_ json: [String: Any]) -> WaveRun? {
        guard let id = json["id"] as? String,
              let flow = json["flow"] as? String,
              let repoPath = json["repo"] as? String else {
            return nil
        }

        let waveId = json["wave_id"] as? String
        let area = normalizeAreaDisplay(json["area"])
        let direction = normalizeDirection(json["direction"])
        let statusStr = json["status"] as? String ?? WaveRunStatus.pending.rawValue
        let status = WaveRunStatus(rawValue: statusStr) ?? .pending
        let iteration = normalizeInt(json["iteration"])
        let stepIndex = normalizeInt(json["step_index"])

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
            stepIndex: stepIndex,
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

    private static func normalizeStringList(_ value: Any?) -> [String] {
        if let list = value as? [String] { return list }
        if let str = value as? String { return decodeStringArray(str) }
        return []
    }

    private static func normalizeAreaDisplay(_ value: Any?) -> String {
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

    private static func normalizeDirection(_ value: Any?) -> [String] {
        normalizeStringList(value)
    }

    private static func normalizeInt(_ value: Any?) -> Int {
        if let intValue = value as? Int { return intValue }
        if let doubleValue = value as? Double { return Int(doubleValue) }
        return 0
    }

    private static func parseDate(_ value: Any?) -> Date? {
        guard let dateStr = value as? String else { return nil }
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        if let date = formatter.date(from: dateStr) {
            return date
        }
        return ISO8601DateFormatter().date(from: dateStr)
    }

    private static func decodeStringArray(_ str: String?) -> [String] {
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

    private static func parseErrorMessage(_ data: Data) -> String? {
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
