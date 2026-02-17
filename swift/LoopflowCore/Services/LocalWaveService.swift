// Service for loading Wave and WaveRun data from lfd daemon (HTTP).

import Foundation

public struct WaveConfigUpdate: Sendable {
    public var name: String?
    public var area: [String]?
    public var direction: [String]?
    public var flow: String?
    public var status: WaveStatus?

    public init(
        name: String? = nil,
        area: [String]? = nil,
        direction: [String]? = nil,
        flow: String? = nil,
        status: WaveStatus? = nil
    ) {
        self.name = name
        self.area = area
        self.direction = direction
        self.flow = flow
        self.status = status
    }
}

public struct RunOverrides: Sendable {
    public var area: [String]?
    public var direction: [String]?
    public var flow: String?

    public init(
        area: [String]? = nil,
        direction: [String]? = nil,
        flow: String? = nil
    ) {
        self.area = area
        self.direction = direction
        self.flow = flow
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

public struct WaveService: WaveServiceProtocol, @unchecked Sendable {
    public typealias SessionFactory = @Sendable (
        _ requestTimeout: TimeInterval,
        _ resourceTimeout: TimeInterval,
        _ delegate: URLSessionDelegate?
    ) -> URLSession

    private let connection: ServerConnection
    private let tokenProvider: @Sendable () -> String?
    private let sessionFactory: SessionFactory
    private let pinStore: CertificatePinStore

    private var pinningDelegate: CertificatePinningDelegate? {
        guard connection.useTLS else { return nil }
        return CertificatePinningDelegate(connection: connection, pinStore: pinStore)
    }

    public init(
        connection: ServerConnection = .local,
        tokenProvider: (@Sendable () -> String?)? = nil,
        sessionFactory: SessionFactory? = nil,
        pinStore: CertificatePinStore = .shared
    ) {
        self.connection = connection
        self.tokenProvider = {
            FileTokenProvider.resolveToken(
                connection: connection,
                tokenProvider: tokenProvider
            )
        }
        self.sessionFactory = sessionFactory ?? { requestTimeout, resourceTimeout, delegate in
            let config = URLSessionConfiguration.default
            config.timeoutIntervalForRequest = requestTimeout
            config.timeoutIntervalForResource = resourceTimeout
            return URLSession(configuration: config, delegate: delegate, delegateQueue: nil)
        }
        self.pinStore = pinStore
    }

    private var baseURL: URL { connection.httpBaseURL }

    private var apiBaseURL: URL {
        baseURL.appendingPathComponent("v0")
    }

    private var standardTimeouts: (request: TimeInterval, resource: TimeInterval) {
        connection.isLocal ? (3, 10) : (10, 30)
    }

    private var longTimeouts: (request: TimeInterval, resource: TimeInterval) {
        connection.isLocal ? (30, 60) : (60, 120)
    }

    private func makeSession(
        requestTimeout: TimeInterval,
        resourceTimeout: TimeInterval
    ) -> (session: URLSession, delegate: CertificatePinningDelegate?) {
        let delegate = pinningDelegate
        return (
            sessionFactory(requestTimeout, resourceTimeout, delegate),
            delegate
        )
    }

    private func standardSession() -> (session: URLSession, delegate: CertificatePinningDelegate?) {
        makeSession(
            requestTimeout: standardTimeouts.request,
            resourceTimeout: standardTimeouts.resource
        )
    }

    private func longSession() -> (session: URLSession, delegate: CertificatePinningDelegate?) {
        makeSession(
            requestTimeout: longTimeouts.request,
            resourceTimeout: longTimeouts.resource
        )
    }

    private func makeRequest(
        _ url: URL,
        method: String = "GET",
        body: [String: Any]? = nil,
        contentType: String? = nil
    ) throws -> URLRequest {
        var request = URLRequest(url: url)
        request.httpMethod = method

        if let contentType {
            request.setValue(contentType, forHTTPHeaderField: "Content-Type")
        }

        if connection.authMode.requiresToken {
            guard let token = tokenProvider(), !token.isEmpty else {
                throw WaveServiceError.authRejected("Missing token")
            }
            request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        } else if connection.isLocal,
                  let token = tokenProvider(),
                  !token.isEmpty {
            request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        }

        if let body {
            request.httpBody = try JSONSerialization.data(withJSONObject: body)
        }

        return request
    }

    private func consumeTrustRequirement(from delegate: CertificatePinningDelegate?) -> WaveServiceError? {
        guard let requirement = delegate?.consumeTrustRequirement(),
              case let .certificateChanged(_, _, oldFingerprint, newFingerprint) = requirement else {
            return nil
        }
        return .trustMismatch(oldFingerprint: oldFingerprint, newFingerprint: newFingerprint)
    }

    private func mapError(_ error: Error, trustDelegate: CertificatePinningDelegate?) -> WaveServiceError {
        if let trustError = consumeTrustRequirement(from: trustDelegate) {
            return trustError
        }

        if let waveError = error as? WaveServiceError {
            return waveError
        }

        if let urlError = error as? URLError {
            switch urlError.code {
            case .timedOut:
                return .timeout
            case .notConnectedToInternet, .networkConnectionLost:
                return .networkUnavailable
            case .cannotFindHost, .cannotConnectToHost, .dnsLookupFailed:
                return .serverUnreachable
            default:
                return .commandFailed(urlError.localizedDescription)
            }
        }

        return .commandFailed(error.localizedDescription)
    }

    private func parseStatusCodeError(statusCode: Int, data: Data) -> WaveServiceError {
        let errorMessage = Self.parseErrorMessage(data)
        switch statusCode {
        case 401, 403:
            return .authRejected(errorMessage)
        default:
            return .serverError(status: statusCode, message: errorMessage)
        }
    }

    private func performRequest(
        _ request: URLRequest,
        useLongTimeouts: Bool = false
    ) async throws -> (Data, HTTPURLResponse) {
        let activeSession = useLongTimeouts ? longSession() : standardSession()
        do {
            let (data, response) = try await activeSession.session.data(for: request)
            guard let httpResponse = response as? HTTPURLResponse else {
                throw WaveServiceError.commandFailed("No response from lfd")
            }
            return (data, httpResponse)
        } catch {
            throw mapError(error, trustDelegate: activeSession.delegate)
        }
    }

    private func performGet(
        _ url: URL,
        useLongTimeouts: Bool = false
    ) async throws -> (Data, HTTPURLResponse) {
        let request = try makeRequest(url)
        return try await performRequest(request, useLongTimeouts: useLongTimeouts)
    }

    private func waveURL(_ waveId: String, components: String...) -> URL {
        components.reduce(
            apiBaseURL
                .appendingPathComponent("waves")
                .appendingPathComponent(waveId)
        ) { url, component in
            url.appendingPathComponent(component)
        }
    }

    // MARK: - Waves

    /// List waves for a repository via HTTP API.
    /// The API normalizes worktree paths to their main repo.
    public func listWaves(repo: RepoTarget) async throws -> [Wave] {
        let repoPath = repo.path
        var components = URLComponents(url: apiBaseURL.appendingPathComponent("waves"), resolvingAgainstBaseURL: false)!
        components.queryItems = [
            URLQueryItem(name: "repo", value: repoPath),
            URLQueryItem(name: "expand[]", value: "active_run"),
        ]

        guard let url = components.url else {
            LoggingService.lfd("listWaves: invalid URL for repo=\(repoPath)")
            return []
        }

        LoggingService.lfd("listWaves: GET \(url)")

        let (data, response) = try await performGet(url, useLongTimeouts: true)

        LoggingService.lfd("listWaves: status=\(response.statusCode)")

        guard response.statusCode == 200 else {
            throw parseStatusCodeError(statusCode: response.statusCode, data: data)
        }

        guard let json = try JSONSerialization.jsonObject(with: data) as? [String: Any],
              let wavesData = json["data"] as? [[String: Any]] else {
            throw WaveServiceError.commandFailed("Invalid JSON response")
        }

        let waves = wavesData.map { Self.parseWaveFromJSON($0) }
        LoggingService.lfd("listWaves: found \(waves.count) waves")
        return waves
    }

    /// Fetch a single wave by id.
    public func getWave(_ id: String) async throws -> Wave {
        var components = URLComponents(url: apiBaseURL.appendingPathComponent("waves/\(id)"), resolvingAgainstBaseURL: false)!
        components.queryItems = [URLQueryItem(name: "expand[]", value: "active_run")]

        guard let url = components.url else {
            throw WaveServiceError.commandFailed("Invalid wave URL")
        }

        LoggingService.lfd("getWave: GET \(url)")

        let (data, response) = try await performGet(url)

        guard response.statusCode == 200 else {
            let errorMsg = Self.parseErrorMessage(data)
            throw WaveServiceError.commandFailed(errorMsg ?? "HTTP \(response.statusCode)")
        }

        guard let json = try JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            throw WaveServiceError.commandFailed("Invalid response")
        }

        return Self.parseWaveFromJSON(json)
    }

    /// List flows and steps from lfd.
    public func listFlowsAndDirections(repo: RepoTarget) async throws -> WaveFlowsResult {
        var components = URLComponents(url: apiBaseURL.appendingPathComponent("flows"), resolvingAgainstBaseURL: false)!
        components.queryItems = [URLQueryItem(name: "repo", value: repo.path)]

        guard let url = components.url else {
            LoggingService.lfd("listFlows: invalid URL")
            return WaveFlowsResult(flows: [], directions: [])
        }

        LoggingService.lfd("listFlows: GET \(url)")

        let (data, response) = try await performGet(url)
        guard response.statusCode == 200 else {
            throw parseStatusCodeError(statusCode: response.statusCode, data: data)
        }
        guard let json = try JSONSerialization.jsonObject(with: data) as? [String: Any],
              let result = json["result"] as? [String: Any] else {
            return WaveFlowsResult(flows: [], directions: [])
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

        return WaveFlowsResult(flows: flows + steps, directions: directions)
    }

    public func listWaveSchemas(repo: RepoTarget) async throws -> [WaveSchema] {
        var components = URLComponents(url: apiBaseURL.appendingPathComponent("wave/schemas"), resolvingAgainstBaseURL: false)!
        components.queryItems = [URLQueryItem(name: "repo", value: repo.path)]

        guard let url = components.url else {
            LoggingService.lfd("listWaveSchemas: invalid URL")
            return []
        }

        LoggingService.lfd("listWaveSchemas: GET \(url)")

        do {
            let (data, response) = try await performGet(url)
            guard response.statusCode == 200 else {
                return []
            }
            guard let json = try JSONSerialization.jsonObject(with: data) as? [String: Any],
                  let schemaData = json["data"] as? [[String: Any]] else {
                return []
            }

            return schemaData.compactMap(Self.parseWaveSchemaFromJSON)
        } catch {
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
            let (data, response) = try await performGet(url)

            guard response.statusCode == 200 else {
                throw parseStatusCodeError(statusCode: response.statusCode, data: data)
            }

            guard let json = try JSONSerialization.jsonObject(with: data) as? [String: Any],
                  let runsData = json["data"] as? [[String: Any]] else {
                LoggingService.lfd("listWaveRuns: invalid JSON response")
                return []
            }

            return runsData.compactMap { Self.parseWaveRunFromJSON($0) }
        } catch {
            LoggingService.lfd("listWaveRuns: error=\(error.localizedDescription)")
            throw error
        }
    }

    // MARK: - Actions

    public func listRepos() async throws -> [RemoteRepo] {
        let url = apiBaseURL.appendingPathComponent("repos")
        let (data, response) = try await performGet(url)

        guard response.statusCode == 200 else {
            throw parseStatusCodeError(statusCode: response.statusCode, data: data)
        }

        guard let json = try JSONSerialization.jsonObject(with: data) as? [String: Any],
              let repos = json["data"] as? [[String: Any]] else {
            throw WaveServiceError.commandFailed("Invalid response")
        }

        return repos.compactMap { repo in
            guard let path = repo["path"] as? String,
                  let name = repo["name"] as? String else {
                return nil
            }
            return RemoteRepo(
                path: path,
                name: name,
                waveCount: Self.normalizeInt(repo["wave_count"])
            )
        }
    }

    public func checkConnection() async throws {
        let url = baseURL.appendingPathComponent("status")
        let (_, response) = try await performGet(url)
        guard response.statusCode == 200 else {
            throw parseStatusCodeError(statusCode: response.statusCode, data: Data())
        }
    }

    public func connectLfd() async throws {
        guard connection.isLocal else {
            try await checkConnection()
            return
        }
        LoggingService.lfd("connectLfd: running 'lfd install'")
        try await runShellCommand(["lfd", "install"])
        LoggingService.lfd("connectLfd: 'lfd install' completed")
    }

    public func checkAvailability() async -> Bool {
        let url = baseURL.appendingPathComponent("status")
        do {
            let (_, response) = try await performGet(url)
            return response.statusCode == 200
        } catch {
            return false
        }
    }

    public func createWave(name: String, repo: RepoTarget, schema: String? = nil) async throws -> Wave {
        LoggingService.lfd("createWave: name=\(name.isEmpty ? "(auto)" : name) repo=\(repo.path)")

        // Create wave via lfd HTTP API with minimal config
        // User configures area, direction, flow in detail panel before running
        // Create with defaults - area left empty, user configures in detail panel
        var body: [String: Any] = [
            "repo": repo.path,
            "name": name.isEmpty ? NSNull() : name,
        ]
        if let schema {
            body["schema"] = schema
        } else {
            body["flow"] = "design"  // Default flow (single step)
            body["direction"] = []
        }
        let request = try makeRequest(
            apiBaseURL.appendingPathComponent("waves"),
            method: "POST",
            body: body,
            contentType: "application/json"
        )

        LoggingService.lfd("createWave: POST \(request.url!) body=\(body)")

        do {
            // Use long timeout - createWave does git fetch + worktree add + push
            let (data, response) = try await performRequest(request, useLongTimeouts: true)

            LoggingService.lfd("createWave: status=\(response.statusCode)")

            guard response.statusCode == 200 else {
                let errorBody = String(data: data, encoding: .utf8) ?? ""
                LoggingService.lfd("createWave: error response=\(errorBody)")
                throw parseStatusCodeError(statusCode: response.statusCode, data: data)
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

        let stimuli: [Stimulus]
        if let stimArr = json["stimuli"] as? [[String: Any]] {
            stimuli = stimArr.compactMap { dict in
                guard let id = dict["id"] as? String,
                      let kindStr = dict["kind"] as? String,
                      let kind = Stimulus.Kind(rawValue: kindStr) else { return nil }
                return Stimulus(id: id, kind: kind, cron: dict["cron"] as? String)
            }
        } else {
            stimuli = []
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

        let flowSteps = json["flow_steps"] as? [String] ?? []

        return Wave(
            id: json["id"] as? String ?? UUID().uuidString,
            name: json["name"] as? String ?? "",
            repo: json["repo"] as? String ?? "",
            flow: json["flow"] as? String ?? "",
            direction: normalizeStringList(json["direction"]),
            area: normalizeStringList(json["area"]),
            stimuli: stimuli,
            status: status,
            iteration: json["iteration"] as? Int ?? 0,
            localWorktree: json["local_worktree"] as? String,
            remoteBranch: json["remote_branch"] as? String,
            commits: commits,
            diffStat: json["diff_stat"] as? String,
            flowSteps: flowSteps,
            openPRCount: normalizeInt(json["open_pr_count"]),
            activeRun: activeRun,
            createdAt: createdAt
        )
    }

    /// Update wave configuration (name, area, direction, flow, status).
    public func updateWave(_ id: String, config: WaveConfigUpdate) async throws -> Wave {
        var body: [String: Any] = [:]
        if let name = config.name { body["name"] = name }
        if let area = config.area { body["area"] = area }
        if let direction = config.direction { body["direction"] = direction }
        if let flow = config.flow { body["flow"] = flow }
        if let status = config.status { body["status"] = status.rawValue }

        let request = try makeRequest(
            apiBaseURL.appendingPathComponent("waves/\(id)"),
            method: "PATCH",
            body: body,
            contentType: "application/json"
        )

        // Rename may move worktree + rename branch + push, so use long timeouts.
        let (data, response) = try await performRequest(request, useLongTimeouts: true)

        guard response.statusCode == 200 else {
            let errorMsg = Self.parseErrorMessage(data)
            throw WaveServiceError.commandFailed(errorMsg ?? "HTTP \(response.statusCode)")
        }

        guard let json = try JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            let errorMsg = Self.parseErrorMessage(data)
            throw WaveServiceError.commandFailed(errorMsg ?? "Invalid response")
        }

        return Self.parseWaveFromJSON(json)
    }

    /// Run wave, optionally with one-time overrides.
    public func run(_ id: String, overrides: RunOverrides?) async throws {
        // Build body with any overrides
        var body: [String: Any] = [:]
        if let overrides {
            if let area = overrides.area { body["area"] = area }
            if let direction = overrides.direction { body["direction"] = direction }
            if let flow = overrides.flow { body["flow"] = flow }
        }

        let request = try makeRequest(
            apiBaseURL.appendingPathComponent("waves/\(id)/run"),
            method: "POST",
            body: body.isEmpty ? nil : body,
            contentType: "application/json"
        )

        let (data, response) = try await performRequest(request)

        guard response.statusCode == 200 else {
            let errorMsg = Self.parseErrorMessage(data)
            throw WaveServiceError.commandFailed(errorMsg ?? "HTTP \(response.statusCode)")
        }

        guard (try? JSONSerialization.jsonObject(with: data)) != nil else {
            let errorMsg = Self.parseErrorMessage(data)
            throw WaveServiceError.commandFailed(errorMsg ?? "Invalid response")
        }
    }

    // MARK: - Stimulus

    public func addStimulus(_ waveId: String, kind: Stimulus.Kind, cron: String? = nil) async throws -> Stimulus {
        var body: [String: Any] = ["kind": kind.rawValue]
        if let cron { body["cron"] = cron }
        let request = try makeRequest(
            apiBaseURL.appendingPathComponent("waves/\(waveId)/stimulus"),
            method: "POST",
            body: body,
            contentType: "application/json"
        )

        let (data, response) = try await performRequest(request)

        guard response.statusCode == 200 else {
            let errorMsg = Self.parseErrorMessage(data)
            throw WaveServiceError.commandFailed(errorMsg ?? "HTTP \(response.statusCode)")
        }

        guard let json = try JSONSerialization.jsonObject(with: data) as? [String: Any],
              let id = json["id"] as? String else {
            throw WaveServiceError.commandFailed("Invalid response")
        }

        return Stimulus(id: id, kind: kind, cron: cron)
    }

    public func removeStimulus(_ waveId: String, stimulusId: String) async throws {
        let request = try makeRequest(
            apiBaseURL.appendingPathComponent("waves/\(waveId)/stimulus/\(stimulusId)"),
            method: "DELETE"
        )

        let (data, response) = try await performRequest(request)

        guard response.statusCode == 200 else {
            let errorMsg = Self.parseErrorMessage(data)
            throw WaveServiceError.commandFailed(errorMsg ?? "HTTP \(response.statusCode)")
        }
    }

    public func listMemoryBlocks(waveId: String) async throws -> [ChatMemoryBlock] {
        let url = waveURL(waveId, components: "memory-blocks")
        let request = try makeRequest(url)

        let (data, response) = try await performRequest(request)
        guard response.statusCode == 200 else {
            throw parseStatusCodeError(statusCode: response.statusCode, data: data)
        }

        guard let json = try JSONSerialization.jsonObject(with: data) as? [String: Any],
              let rows = json["data"] as? [[String: Any]] else {
            throw WaveServiceError.commandFailed("Invalid response")
        }

        return rows.compactMap(Self.parseMemoryBlockFromJSON)
    }

    public func upsertMemoryBlock(
        waveId: String,
        name: String,
        content: String,
        position: Int?
    ) async throws -> ChatMemoryBlock {
        let url = waveURL(waveId, components: "memory-blocks", name)

        var body: [String: Any] = ["content": content]
        if let position { body["position"] = position }
        let request = try makeRequest(
            url,
            method: "PUT",
            body: body,
            contentType: "application/json"
        )

        let (data, response) = try await performRequest(request)
        guard response.statusCode == 200 else {
            throw parseStatusCodeError(statusCode: response.statusCode, data: data)
        }

        guard let json = try JSONSerialization.jsonObject(with: data) as? [String: Any],
              let block = Self.parseMemoryBlockFromJSON(json) else {
            throw WaveServiceError.commandFailed("Invalid response")
        }
        return block
    }

    public func deleteMemoryBlock(waveId: String, name: String) async throws {
        let url = waveURL(waveId, components: "memory-blocks", name)

        let request = try makeRequest(url, method: "DELETE")
        let (data, response) = try await performRequest(request)
        guard response.statusCode == 200 else {
            throw parseStatusCodeError(statusCode: response.statusCode, data: data)
        }
    }

    public func startChat(
        waveId: String,
        message: String,
        memoryBlocks: [ChatMemoryBlock]
    ) async throws {
        let url = waveURL(waveId, components: "chat")

        let body: [String: Any] = [
            "message": message,
            "memory_blocks": memoryBlocks.map { block in
                [
                    "name": block.name,
                    "content": block.content,
                    "position": block.position,
                ]
            },
        ]

        let request = try makeRequest(
            url,
            method: "POST",
            body: body,
            contentType: "application/json"
        )

        let (data, response) = try await performRequest(request, useLongTimeouts: true)
        guard response.statusCode == 200 else {
            throw parseStatusCodeError(statusCode: response.statusCode, data: data)
        }
    }

    public func streamChatEvents(
        waveId: String
    ) -> AsyncThrowingStream<ChatTurnEvent, Error> {
        AsyncThrowingStream { continuation in
            Task {
                let activeSession = longSession()
                do {
                    let url = waveURL(waveId, components: "chat", "events")
                    let request = try makeRequest(url)
                    let (bytes, response) = try await activeSession.session.bytes(for: request)

                    guard let httpResponse = response as? HTTPURLResponse else {
                        throw WaveServiceError.commandFailed("No response from lfd")
                    }

                    guard httpResponse.statusCode == 200 else {
                        var data = Data()
                        for try await byte in bytes {
                            data.append(byte)
                        }
                        throw parseStatusCodeError(statusCode: httpResponse.statusCode, data: data)
                    }

                    for try await line in bytes.lines {
                        guard line.hasPrefix("data:") else { continue }
                        let payload = String(line.dropFirst(5)).trimmingCharacters(in: .whitespaces)
                        guard !payload.isEmpty else { continue }
                        let event = try Self.parseChatTurnEvent(payload)
                        continuation.yield(event)
                        if event.isTerminal {
                            break
                        }
                    }
                    continuation.finish()
                } catch {
                    continuation.finish(throwing: mapError(error, trustDelegate: activeSession.delegate))
                }
            }
        }
    }

    public func connect(_ id: String) async throws -> ConnectionInfo {
        let request = try makeRequest(
            apiBaseURL.appendingPathComponent("waves/\(id)/connect"),
            method: "POST"
        )

        let (data, response) = try await performRequest(request)

        guard response.statusCode == 200 else {
            let errorMsg = (try? JSONSerialization.jsonObject(with: data) as? [String: Any])?["detail"] as? String
            throw WaveServiceError.commandFailed(errorMsg ?? "HTTP \(response.statusCode)")
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
        try await postWaveCommand(id, action: "stop")
    }

    public func restartStep(_ id: String) async throws {
        try await postWaveCommand(id, action: "restart-step")
    }

    /// Land a wave's current branch (merge via PR).
    public func landWave(_ id: String) async throws {
        let request = try makeRequest(
            apiBaseURL.appendingPathComponent("waves/\(id)/land"),
            method: "POST",
            body: ["create_pr": true],
            contentType: "application/json"
        )

        let (data, response) = try await performRequest(request, useLongTimeouts: true)

        guard response.statusCode == 200 else {
            let errorMsg = Self.parseErrorMessage(data)
            throw WaveServiceError.commandFailed(errorMsg ?? "HTTP \(response.statusCode)")
        }
    }

    /// Start the next iteration of a wave (create new branch).
    public func nextWave(_ id: String) async throws -> String {
        let request = try makeRequest(
            apiBaseURL.appendingPathComponent("waves/\(id)/next"),
            method: "POST"
        )

        let (data, response) = try await performRequest(request, useLongTimeouts: true)

        guard response.statusCode == 200 else {
            let errorMsg = Self.parseErrorMessage(data)
            throw WaveServiceError.commandFailed(errorMsg ?? "HTTP \(response.statusCode)")
        }

        guard let json = try JSONSerialization.jsonObject(with: data) as? [String: Any],
              let newBranch = json["new_branch"] as? String else {
            throw WaveServiceError.commandFailed("Invalid response")
        }

        return newBranch
    }

    /// Clone a wave with a new name.
    public func cloneWave(_ id: String, name: String?) async throws -> Wave {
        let request = try makeRequest(
            apiBaseURL.appendingPathComponent("waves/\(id)/clone"),
            method: "POST",
            body: name.map { ["name": $0] },
            contentType: "application/json"
        )

        let (data, response) = try await performRequest(request)

        guard response.statusCode == 200 else {
            let errorMsg = Self.parseErrorMessage(data)
            throw WaveServiceError.commandFailed(errorMsg ?? "HTTP \(response.statusCode)")
        }

        guard let json = try JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            let errorMsg = Self.parseErrorMessage(data)
            throw WaveServiceError.commandFailed(errorMsg ?? "Invalid response")
        }

        return Self.parseWaveFromJSON(json)
    }

    /// Delete a wave.
    public func deleteWave(_ id: String) async throws {
        let request = try makeRequest(
            apiBaseURL.appendingPathComponent("waves/\(id)"),
            method: "DELETE"
        )

        let (data, response) = try await performRequest(request)

        guard response.statusCode == 200 else {
            let errorMsg = Self.parseErrorMessage(data)
            throw WaveServiceError.commandFailed(errorMsg ?? "HTTP \(response.statusCode)")
        }

        guard (try? JSONSerialization.jsonObject(with: data)) != nil else {
            let errorMsg = Self.parseErrorMessage(data)
            throw WaveServiceError.commandFailed(errorMsg ?? "Invalid response")
        }
    }

    /// Combine all outstanding PRs for a wave into a single PR.
    /// Returns the new PR URL on success.
    public func combinePRs(_ id: String) async throws -> CombinePRsResult {
        let request = try makeRequest(
            apiBaseURL.appendingPathComponent("waves/\(id)/combine"),
            method: "POST"
        )

        // Use long timeouts - combine does multiple git/gh operations
        let (data, response) = try await performRequest(request, useLongTimeouts: true)

        guard response.statusCode == 200 else {
            let errorMsg = Self.parseErrorMessage(data)
            throw WaveServiceError.commandFailed(errorMsg ?? "HTTP \(response.statusCode)")
        }

        guard let json = try JSONSerialization.jsonObject(with: data) as? [String: Any],
              let ok = json["ok"] as? Bool else {
            throw WaveServiceError.commandFailed("Invalid response")
        }

        if ok, let result = json["result"] as? [String: Any] {
            let newPRUrl = result["new_pr_url"] as? String
            let closedPRs = result["closed_prs"] as? [Int] ?? []
            return CombinePRsResult(newPRUrl: newPRUrl, closedPRs: closedPRs)
        } else {
            let errorMsg = json["error"] as? String ?? "Combine failed"
            throw WaveServiceError.commandFailed(errorMsg)
        }
    }

    // MARK: - Worktrees

    public func listWorktrees(repo: RepoTarget) async throws -> [WorktreeInfo] {
        guard case .local = repo else { return [] }
        var components = URLComponents(url: apiBaseURL.appendingPathComponent("worktrees"), resolvingAgainstBaseURL: false)!
        components.queryItems = [URLQueryItem(name: "repo", value: repo.path)]

        guard let url = components.url else {
            LoggingService.lfd("listWorktrees: invalid URL for repo=\(repo.path)")
            return []
        }

        LoggingService.lfd("listWorktrees: GET \(url)")

        do {
            let (data, response) = try await performGet(url, useLongTimeouts: true)

            guard response.statusCode == 200 else {
                throw parseStatusCodeError(statusCode: response.statusCode, data: data)
            }

            guard let json = try JSONSerialization.jsonObject(with: data) as? [String: Any],
                  let items = json["data"] as? [[String: Any]] else {
                LoggingService.lfd("listWorktrees: invalid JSON response")
                return []
            }

            let worktrees = items.compactMap(Self.parseWorktreeFromJSON)
            LoggingService.lfd("listWorktrees: found \(worktrees.count) worktrees")
            return worktrees
        } catch {
            LoggingService.lfd("listWorktrees: error=\(error.localizedDescription)")
            throw error
        }
    }

    // MARK: - Output Streaming

    /// Stream output for a wave (replay from disk + live follow).
    /// Yields one line at a time. The connection stays open for live updates.
    public func streamOutput(waveId: String) -> AsyncThrowingStream<String, Error> {
        let url = apiBaseURL.appendingPathComponent("waves/\(waveId)/logs")

        return AsyncThrowingStream { continuation in
            let task = Task {
                let delegate = self.pinningDelegate
                do {
                    let config = URLSessionConfiguration.default
                    config.timeoutIntervalForRequest = 300
                    config.timeoutIntervalForResource = 86400 // Keep alive for a day
                    let streamSession = URLSession(configuration: config, delegate: delegate, delegateQueue: nil)
                    let request = try self.makeRequest(url)

                    let (bytes, response) = try await streamSession.bytes(for: request)

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
                    if let trustError = self.consumeTrustRequirement(from: delegate) {
                        continuation.finish(throwing: trustError)
                        return
                    }
                    continuation.finish(throwing: error)
                }
            }

            continuation.onTermination = { _ in
                task.cancel()
            }
        }
    }

    // MARK: - Private helpers

    private static func parseWaveSchemaFromJSON(_ json: [String: Any]) -> WaveSchema? {
        guard let schemaRef = json["schema_ref"] as? String,
              let name = json["name"] as? String,
              let flow = json["flow"] as? String,
              let sourceRaw = json["source"] as? String,
              let source = WaveSchema.Source(rawValue: sourceRaw) else {
            return nil
        }

        let stimulus: StimulusSchema?
        if let stimulusJson = json["stimulus"] as? [String: Any],
           let kind = stimulusJson["kind"] as? String {
            stimulus = StimulusSchema(kind: kind, cron: stimulusJson["cron"] as? String)
        } else {
            stimulus = nil
        }

        return WaveSchema(
            schemaRef: schemaRef,
            name: name,
            flow: flow,
            area: normalizeStringList(json["area"]),
            direction: normalizeDirection(json["direction"]),
            owner: json["owner"] as? String,
            description: json["description"] as? String,
            source: source,
            activeWaveId: json["active_wave_id"] as? String,
            stimulus: stimulus
        )
    }

    private static func parseWorktreeFromJSON(_ json: [String: Any]) -> WorktreeInfo? {
        guard let path = json["path"] as? String, !path.isEmpty else { return nil }

        return WorktreeInfo(
            branch: json["branch"] as? String,
            path: path,
            merged: json["merged"] as? Bool ?? false,
            prunable: json["prunable"] as? Bool ?? false,
            waveId: json["wave_id"] as? String
        )
    }

    private static func parseMemoryBlockFromJSON(_ json: [String: Any]) -> ChatMemoryBlock? {
        guard let name = json["name"] as? String,
              let content = json["content"] as? String else {
            return nil
        }

        return ChatMemoryBlock(
            name: name,
            content: content,
            position: normalizeInt(json["position"]),
            updatedAt: parseDate(json["updated_at"])
        )
    }

    private static func parseChatTurnEvent(_ raw: String) throws -> ChatTurnEvent {
        guard let data = raw.data(using: .utf8),
              let json = try JSONSerialization.jsonObject(with: data) as? [String: Any],
              let type = json["type"] as? String else {
            throw WaveServiceError.commandFailed("Invalid chat event payload")
        }

        switch type {
        case "message":
            let content = json["content"] as? String ?? ""
            let phaseRaw = json["phase"] as? String
            let phase = ChatTurnPhase(rawValue: phaseRaw ?? "") ?? .progress
            return .message(content: content, phase: phase)
        case "memory_edit":
            return .memoryEdit(
                op: json["op"] as? String ?? "upsert",
                block: json["block"] as? String ?? "",
                detail: json["detail"] as? String ?? ""
            )
        case "done":
            return .done
        case "failed":
            return .failed(
                code: json["code"] as? String ?? "failed",
                message: json["message"] as? String ?? "Turn failed"
            )
        default:
            throw WaveServiceError.commandFailed("Unknown chat event type: \(type)")
        }
    }

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

    private func postWaveCommand(_ id: String, action: String) async throws {
        let request = try makeRequest(
            apiBaseURL.appendingPathComponent("waves/\(id)/\(action)"),
            method: "POST"
        )

        let (data, response) = try await performRequest(request)

        guard response.statusCode == 200 else {
            let errorMsg = Self.parseErrorMessage(data)
            throw WaveServiceError.commandFailed(errorMsg ?? "HTTP \(response.statusCode)")
        }
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
    case authRejected(String?)
    case serverError(status: Int, message: String?)
    case timeout
    case networkUnavailable
    case serverUnreachable
    case trustMismatch(oldFingerprint: String, newFingerprint: String)

    public var errorDescription: String? {
        switch self {
        case .commandFailed(let message):
            return message
        case .authRejected(let message):
            return message ?? "Authentication failed"
        case .serverError(let status, let message):
            return message ?? "Server error (\(status))"
        case .timeout:
            return "Connection timed out"
        case .networkUnavailable:
            return "Network unavailable"
        case .serverUnreachable:
            return "Server unreachable"
        case .trustMismatch:
            return "Certificate fingerprint changed"
        }
    }
}

public struct CombinePRsResult: Sendable {
    public let newPRUrl: String?
    public let closedPRs: [Int]

    public init(newPRUrl: String?, closedPRs: [Int]) {
        self.newPRUrl = newPRUrl
        self.closedPRs = closedPRs
    }
}

public typealias LocalWaveService = WaveService
