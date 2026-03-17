// Service for loading Wave and WaveRun data from lfd daemon (HTTP).

import Foundation

public struct WaveConfigUpdate: Sendable {
    public var name: String?
    public var area: [String]?
    public var direction: [String]?
    public var flow: String?
    public var status: WaveStatus?
    public var agent: String?
    public var stepAgents: [String: String]?

    public init(
        name: String? = nil,
        area: [String]? = nil,
        direction: [String]? = nil,
        flow: String? = nil,
        status: WaveStatus? = nil,
        agent: String? = nil,
        stepAgents: [String: String]? = nil
    ) {
        self.name = name
        self.area = area
        self.direction = direction
        self.flow = flow
        self.status = status
        self.agent = agent
        self.stepAgents = stepAgents
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

// SAFETY: WaveService is a value type with immutable captured dependencies;
// request state is per-call and not shared across concurrent tasks.
public struct WaveService: WaveServiceProtocol, @unchecked Sendable {
    public typealias SessionFactory = @Sendable (
        _ requestTimeout: TimeInterval,
        _ resourceTimeout: TimeInterval,
        _ delegate: URLSessionDelegate?
    ) -> URLSession
    public typealias ShellCommandRunner = @Sendable (_ args: [String]) async throws -> Void

    private let connection: ServerConnection
    private let tokenProvider: @Sendable () -> String?
    private let sessionFactory: SessionFactory
    private let pinStore: CertificatePinStore
    private let shellCommandRunner: ShellCommandRunner?

    private var pinningDelegate: CertificatePinningDelegate? {
        guard connection.useTLS else { return nil }
        return CertificatePinningDelegate(connection: connection, pinStore: pinStore)
    }

    public init(
        connection: ServerConnection = .local,
        tokenProvider: (@Sendable () -> String?)? = nil,
        sessionFactory: SessionFactory? = nil,
        pinStore: CertificatePinStore = .shared,
        shellCommandRunner: ShellCommandRunner? = nil
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
        self.shellCommandRunner = shellCommandRunner
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
        case 502:
            return .serverUnreachable
        case 504:
            return .daemonTimeout(errorMessage)
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

    private func sessionURL(_ sessionId: String? = nil, components: String...) -> URL {
        var url = apiBaseURL.appendingPathComponent("sessions")
        if let sessionId {
            url = url.appendingPathComponent(sessionId)
        }
        return components.reduce(url) { current, component in
            current.appendingPathComponent(component)
        }
    }

    private func attentionURL(_ attentionId: String? = nil, components: String...) -> URL {
        var url = apiBaseURL.appendingPathComponent("attention")
        if let attentionId {
            url = url.appendingPathComponent(attentionId)
        }
        return components.reduce(url) { current, component in
            current.appendingPathComponent(component)
        }
    }

    private func authURL(_ provider: AuthProvider? = nil) -> URL {
        var url = apiBaseURL.appendingPathComponent("auth")
        if let provider {
            url = url.appendingPathComponent(provider.rawValue)
        }
        return url
    }

    private func requireOKStatus(_ response: HTTPURLResponse, data: Data) throws {
        guard response.statusCode == 200 else {
            throw parseStatusCodeError(statusCode: response.statusCode, data: data)
        }
    }

    private func decodeResponse<T: Decodable>(_ type: T.Type, from data: Data) throws -> T {
        do {
            return try JSONDecoder().decode(T.self, from: data)
        } catch {
            throw WaveServiceError.commandFailed("Invalid response")
        }
    }

    // MARK: - Provider auth

    public func listAuthProviders() async throws -> [AuthProviderStatus] {
        let (data, response) = try await performGet(authURL())
        try requireOKStatus(response, data: data)
        let decoded = try decodeResponse(AuthProviderListResponse.self, from: data)
        return decoded.providers
    }

    public func startAuthFlow(provider: AuthProvider) async throws -> AuthFlow {
        let request = try makeRequest(authURL(provider), method: "POST")
        let (data, response) = try await performRequest(request, useLongTimeouts: true)
        try requireOKStatus(response, data: data)
        return try decodeResponse(AuthFlow.self, from: data)
    }

    public func disconnectProvider(provider: AuthProvider) async throws -> AuthProviderStatus {
        let request = try makeRequest(authURL(provider), method: "DELETE")
        let (data, response) = try await performRequest(request)
        try requireOKStatus(response, data: data)
        return try decodeResponse(AuthProviderStatus.self, from: data)
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
            return WaveFlowsResult(flows: [], directions: [], supportedHarnesses: [])
        }

        LoggingService.lfd("listFlows: GET \(url)")

        let (data, response) = try await performGet(url)
        guard response.statusCode == 200 else {
            throw parseStatusCodeError(statusCode: response.statusCode, data: data)
        }
        guard let json = try JSONSerialization.jsonObject(with: data) as? [String: Any],
              let result = json["result"] as? [String: Any] else {
            return WaveFlowsResult(flows: [], directions: [], supportedHarnesses: [])
        }

        let stepsData = (result["steps"] as? [[String: Any]]) ?? []
        let stepConfigs = stepsData.reduce(into: [String: StepConfig]()) { partial, stepDict in
            guard let name = stepDict["name"] as? String else { return }
            let config = StepConfig(
                agent: stepDict["agent"] as? String,
                defaultAgent: stepDict["default_agent"] as? String
            )
            if !config.isEmpty {
                partial[name] = config
            }
        }
        let stepFromName: (String) -> Step = { name in
            Step(prompt: name, config: stepConfigs[name])
        }

        var allFlows: [Flow] = []
        if let flowsData = result["flows"] as? [[String: Any]] {
            for flowDict in flowsData {
                guard let name = flowDict["name"] as? String else { continue }
                let stepNames = flowDict["steps"] as? [String] ?? []
                let steps = stepNames.map(stepFromName)
                allFlows.append(Flow(name: name, steps: steps, type: .flow))
            }
        }

        for stepDict in stepsData {
            guard let name = stepDict["name"] as? String else { continue }
            allFlows.append(
                Flow(name: name, steps: [stepFromName(name)], type: .step)
            )
        }

        let flows = allFlows.filter { $0.type == .flow }.sorted { $0.name < $1.name }
        let steps = allFlows.filter { $0.type == .step }.sorted { $0.name < $1.name }
        let directions = (result["directions"] as? [String]) ?? []
        let supportedHarnesses = (result["supported_harnesses"] as? [String]) ?? []

        return WaveFlowsResult(
            flows: flows + steps,
            directions: directions,
            supportedHarnesses: supportedHarnesses
        )
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

    public func listAttention(repo: RepoTarget) async throws -> [AttentionItem] {
        var components = URLComponents(url: attentionURL(), resolvingAgainstBaseURL: false)!
        components.queryItems = [URLQueryItem(name: "repo", value: repo.path)]
        guard let url = components.url else {
            throw WaveServiceError.commandFailed("Invalid attention URL")
        }
        let (data, response) = try await performGet(url)
        try requireOKStatus(response, data: data)
        guard let json = try JSONSerialization.jsonObject(with: data) as? [String: Any],
              let items = json["data"] as? [[String: Any]] else {
            throw WaveServiceError.commandFailed("Invalid response")
        }
        return items.compactMap(Self.parseAttentionFromJSON)
    }

    public func getAttention(_ id: String) async throws -> AttentionItem {
        let (data, response) = try await performGet(attentionURL(id))
        try requireOKStatus(response, data: data)
        guard let json = try JSONSerialization.jsonObject(with: data) as? [String: Any],
              let item = Self.parseAttentionFromJSON(json) else {
            throw WaveServiceError.commandFailed("Invalid response")
        }
        return item
    }

    public func markAttentionViewed(_ id: String) async throws -> AttentionItem {
        let request = try makeRequest(
            attentionURL(id),
            method: "PATCH",
            body: [:],
            contentType: "application/json"
        )
        let (data, response) = try await performRequest(request)
        try requireOKStatus(response, data: data)
        guard let json = try JSONSerialization.jsonObject(with: data) as? [String: Any],
              let item = Self.parseAttentionFromJSON(json) else {
            throw WaveServiceError.commandFailed("Invalid response")
        }
        return item
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

        return repos.compactMap(Self.parseRemoteRepo)
    }

    public func addRepo(path: String) async throws -> RemoteRepo {
        let body: [String: Any] = ["path": path]
        let request = try makeRequest(
            apiBaseURL.appendingPathComponent("repos"),
            method: "POST",
            body: body,
            contentType: "application/json"
        )

        let (data, response) = try await performRequest(request)
        guard response.statusCode == 201 else {
            throw parseStatusCodeError(statusCode: response.statusCode, data: data)
        }

        guard let json = try JSONSerialization.jsonObject(with: data) as? [String: Any],
              let repo = Self.parseRemoteRepo(json) else {
            throw WaveServiceError.commandFailed("Invalid response")
        }
        return repo
    }

    public func removeRepo(path: String) async throws {
        let body: [String: Any] = ["path": path]
        let request = try makeRequest(
            apiBaseURL.appendingPathComponent("repos"),
            method: "DELETE",
            body: body,
            contentType: "application/json"
        )
        let (data, response) = try await performRequest(request)
        guard response.statusCode == 204 else {
            throw parseStatusCodeError(statusCode: response.statusCode, data: data)
        }
    }

    public func fileDiff(waveId: String, path: String) async throws -> String {
        let url = baseURL
            .appendingPathComponent("waves")
            .appendingPathComponent(waveId)
            .appendingPathComponent("diff")
        var components = URLComponents(url: url, resolvingAgainstBaseURL: false)!
        components.queryItems = [URLQueryItem(name: "path", value: path)]
        let requestURL = components.url!
        let (data, response) = try await performGet(requestURL)
        guard response.statusCode == 200 else {
            throw parseStatusCodeError(statusCode: response.statusCode, data: data)
        }
        let json = try JSONSerialization.jsonObject(with: data) as? [String: Any]
        return json?["diff"] as? String ?? ""
    }

    public func usageSummary(
        filters: UsageAnalyticsFilters,
        groupBy: UsageGroupBy
    ) async throws -> UsageSummary {
        var components = URLComponents(
            url: apiBaseURL.appendingPathComponent("usage/summary"),
            resolvingAgainstBaseURL: false
        )
        components?.queryItems = Self.usageQueryItems(
            filters: filters,
            groupBy: groupBy,
            bucket: nil
        )
        guard let url = components?.url else {
            throw WaveServiceError.commandFailed("Invalid usage summary URL")
        }

        let (data, response) = try await performGet(url)
        try requireOKStatus(response, data: data)
        return try decodeResponse(UsageSummary.self, from: data)
    }

    public func usageTimeseries(
        filters: UsageAnalyticsFilters,
        bucket: UsageTimeBucket,
        groupBy: UsageGroupBy
    ) async throws -> UsageTimeseries {
        var components = URLComponents(
            url: apiBaseURL.appendingPathComponent("usage/timeseries"),
            resolvingAgainstBaseURL: false
        )
        components?.queryItems = Self.usageQueryItems(
            filters: filters,
            groupBy: groupBy,
            bucket: bucket
        )
        guard let url = components?.url else {
            throw WaveServiceError.commandFailed("Invalid usage timeseries URL")
        }

        let (data, response) = try await performGet(url)
        try requireOKStatus(response, data: data)
        return try decodeResponse(UsageTimeseries.self, from: data)
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
        guard let shellCommandRunner else {
            throw WaveServiceError.commandFailed(
                "Local lfd bootstrap is unavailable on this platform."
            )
        }
        LoggingService.lfd("connectLfd: running 'lfd install'")
        try await shellCommandRunner(["lfd", "install"])
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

    public func createWave(name: String, repo: RepoTarget) async throws -> Wave {
        try await createWave(name: name, repo: repo, flow: "ship-roadmap", run: false)
    }

    public func createWave(name: String, repo: RepoTarget, flow: String, run: Bool) async throws -> Wave {
        LoggingService.lfd(
            "createWave: name=\(name.isEmpty ? "(auto)" : name) repo=\(repo.path) flow=\(flow) run=\(run)"
        )

        let body: [String: Any] = [
            "repo": repo.path,
            "name": name.isEmpty ? NSNull() : name,
            "flow": flow,
            "direction": [],
            "run": run,
        ]
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

    static func parseAttentionFromJSON(_ json: [String: Any]) -> AttentionItem? {
        guard let id = json["id"] as? String,
              let waveId = json["wave_id"] as? String,
              let kindRaw = json["kind"] as? String,
              let kind = AttentionKind(rawValue: kindRaw),
              let statusRaw = json["status"] as? String,
              let status = AttentionStatus(rawValue: statusRaw),
              let title = json["title"] as? String,
              let summary = json["summary"] as? String else {
            return nil
        }

        return AttentionItem(
            id: id,
            waveId: waveId,
            runId: json["run_id"] as? String,
            kind: kind,
            status: status,
            title: title,
            summary: summary,
            context: AttentionItem.context(kind: kind, json: json["context"] as? [String: Any] ?? [:]),
            surfacedAt: parseDate(json["surfaced_at"]) ?? Date(),
            viewedAt: parseDate(json["viewed_at"]),
            resolvedAt: parseDate(json["resolved_at"])
        )
    }

    static func parseWaveFromJSON(_ json: [String: Any]) -> Wave {
        let dateFormatter = ISO8601DateFormatter()
        dateFormatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]

        let triggers: [Trigger]
        if let trigArr = json["triggers"] as? [[String: Any]] {
            triggers = trigArr.compactMap { dict in
                guard let id = dict["id"] as? String,
                      let signalStr = dict["signal"] as? String,
                      let signal = Trigger.Signal(rawValue: signalStr) else { return nil }
                return Trigger(
                    id: id,
                    signal: signal,
                    flow: dict["flow"] as? String,
                    sourceWaveId: dict["source_wave_id"] as? String
                )
            }
        } else {
            triggers = []
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
        let stepAgents = (json["step_agents"] as? [String: Any])?.reduce(into: [String: String]()) { partial, entry in
            if let value = entry.value as? String {
                partial[entry.key] = value
            }
        }

        return Wave(
            id: json["id"] as? String ?? UUID().uuidString,
            name: json["name"] as? String ?? "",
            repo: json["repo"] as? String ?? "",
            flow: json["primary_flow"] as? String ?? "",
            direction: normalizeStringList(json["direction"]),
            area: normalizeStringList(json["area"]),
            agent: json["agent"] as? String,
            stepAgents: stepAgents,
            triggers: triggers,
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

    /// Update wave configuration (name, area, direction, flow, status, agent).
    public func updateWave(_ id: String, config: WaveConfigUpdate) async throws -> Wave {
        var body: [String: Any] = [:]
        if let name = config.name { body["name"] = name }
        if let area = config.area { body["area"] = area }
        if let direction = config.direction { body["direction"] = direction }
        if let flow = config.flow { body["flow"] = flow }
        if let status = config.status { body["status"] = status.rawValue }
        if let agent = config.agent { body["agent"] = agent }
        if let stepAgents = config.stepAgents { body["step_agents"] = stepAgents }

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

    // MARK: - Triggers

    public func addTrigger(_ waveId: String, signal: Trigger.Signal, flow: String? = nil) async throws -> Trigger {
        var body: [String: Any] = ["signal": signal.rawValue]
        if let flow { body["flow"] = flow }
        let request = try makeRequest(
            apiBaseURL.appendingPathComponent("waves/\(waveId)/triggers"),
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

        return Trigger(id: id, signal: signal, flow: flow)
    }

    public func removeTrigger(_ waveId: String, triggerId: String) async throws {
        let request = try makeRequest(
            apiBaseURL.appendingPathComponent("waves/\(waveId)/triggers/\(triggerId)"),
            method: "DELETE"
        )

        let (data, response) = try await performRequest(request)

        guard response.statusCode == 200 else {
            let errorMsg = Self.parseErrorMessage(data)
            throw WaveServiceError.commandFailed(errorMsg ?? "HTTP \(response.statusCode)")
        }
    }

    public func createSession(
        harness: String,
        waveRunId: String?,
        config: AgentSessionConfig
    ) async throws -> AgentSession {
        let url = sessionURL()
        var payload = Self.sessionConfigJSON(config)
        payload["harness"] = harness
        if let waveRunId {
            payload["wave_run_id"] = waveRunId
        }

        let request = try makeRequest(
            url,
            method: "POST",
            body: payload,
            contentType: "application/json"
        )
        let (data, response) = try await performRequest(request, useLongTimeouts: true)
        return try parseSessionResponse(data: data, response: response)
    }

    public func getSession(_ id: String) async throws -> AgentSession {
        let request = try makeRequest(sessionURL(id))
        let (data, response) = try await performRequest(request, useLongTimeouts: true)
        return try parseSessionResponse(data: data, response: response)
    }

    public func sendSessionInput(sessionId: String, content: String) async throws -> AgentSession {
        let request = try makeRequest(
            sessionURL(sessionId, components: "input"),
            method: "POST",
            body: ["content": content],
            contentType: "application/json"
        )
        let (data, response) = try await performRequest(request, useLongTimeouts: true)
        return try parseSessionResponse(data: data, response: response)
    }

    public func streamSessionEvents(
        sessionId: String,
        afterSeq: Int?
    ) -> AsyncThrowingStream<AgentSessionEventEnvelope, Error> {
        AsyncThrowingStream { continuation in
            Task {
                let activeSession = longSession()
                do {
                    var components = URLComponents(
                        url: sessionURL(sessionId, components: "events"),
                        resolvingAgainstBaseURL: false
                    )!
                    if let afterSeq {
                        components.queryItems = [URLQueryItem(name: "after_seq", value: String(afterSeq))]
                    }

                    guard let url = components.url else {
                        throw WaveServiceError.commandFailed("Invalid events URL")
                    }

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

                    var pendingSeq: Int?
                    var pendingEventType: String?
                    for try await line in bytes.lines {
                        if line.hasPrefix("id:") {
                            let value = String(line.dropFirst(3)).trimmingCharacters(in: .whitespaces)
                            pendingSeq = Int(value)
                            continue
                        }

                        if line.hasPrefix("event:") {
                            pendingEventType = String(line.dropFirst(6)).trimmingCharacters(in: .whitespaces)
                            continue
                        }

                        guard line.hasPrefix("data:") else { continue }
                        let payload = String(line.dropFirst(5)).trimmingCharacters(in: .whitespaces)
                        guard !payload.isEmpty else { continue }

                        if pendingEventType == "session.replay_completed" {
                            let lastSeq = Self.parseReplayCompletedLastSeq(payload)
                            continuation.yield(AgentSessionEventEnvelope(replayCompletedLastSeq: lastSeq))
                            pendingSeq = nil
                            pendingEventType = nil
                            continue
                        }

                        let event = try Self.parseSessionEvent(payload)
                        continuation.yield(AgentSessionEventEnvelope(seq: pendingSeq, event: event))
                        pendingSeq = nil
                        pendingEventType = nil
                    }

                    continuation.finish()
                } catch {
                    continuation.finish(throwing: mapError(error, trustDelegate: activeSession.delegate))
                }
            }
        }
    }

    public func stopSession(_ id: String) async throws -> AgentSession {
        let request = try makeRequest(sessionURL(id), method: "DELETE")
        let (data, response) = try await performRequest(request)
        return try parseSessionResponse(data: data, response: response)
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

    private func parseSessionResponse(
        data: Data,
        response: HTTPURLResponse
    ) throws -> AgentSession {
        guard response.statusCode == 200 else {
            throw parseStatusCodeError(statusCode: response.statusCode, data: data)
        }
        guard let json = try JSONSerialization.jsonObject(with: data) as? [String: Any],
              let session = Self.parseSessionFromJSON(json) else {
            throw WaveServiceError.commandFailed("Invalid response")
        }
        return session
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

    private static func parseSessionFromJSON(_ json: [String: Any]) -> AgentSession? {
        guard let id = json["id"] as? String,
              let harness = json["harness"] as? String,
              let status = json["status"] as? String else {
            return nil
        }

        let configJSON = json["config"] as? [String: Any] ?? [:]
        let config = AgentSessionConfig(
            step: configJSON["step"] as? String ?? "",
            repoRoot: configJSON["repo_root"] as? String ?? "",
            directions: configJSON["directions"] as? [String] ?? [],
            area: configJSON["area"] as? String,
            wave: configJSON["wave"] as? String,
            message: configJSON["message"] as? String,
            agent: configJSON["agent"] as? String,
            cwd: configJSON["cwd"] as? String,
            maxTurns: normalizeOptionalInt(configJSON["max_turns"]),
            yoloMode: configJSON["yolo_mode"] as? Bool ?? false,
            clientHasUI: configJSON["client_has_ui"] as? Bool,
            clientCompact: configJSON["client_compact"] as? Bool
        )

        return AgentSession(
            id: id,
            harness: harness,
            status: status,
            waveRunId: json["wave_run_id"] as? String,
            providerSessionId: json["provider_session_id"] as? String,
            config: config,
            createdAt: parseDate(json["created_at"]),
            endedAt: parseDate(json["ended_at"])
        )
    }

    private static func sessionConfigJSON(_ config: AgentSessionConfig) -> [String: Any] {
        var json: [String: Any] = [
            "step": config.step,
            "repo_root": config.repoRoot,
            "directions": config.directions,
            "yolo_mode": config.yoloMode,
        ]
        if let area = config.area { json["area"] = area }
        if let wave = config.wave { json["wave"] = wave }
        if let message = config.message { json["message"] = message }
        if let agent = config.agent { json["agent"] = agent }
        if let cwd = config.cwd { json["cwd"] = cwd }
        if let maxTurns = config.maxTurns { json["max_turns"] = maxTurns }
        if let clientHasUI = config.clientHasUI { json["client_has_ui"] = clientHasUI }
        if let clientCompact = config.clientCompact { json["client_compact"] = clientCompact }
        return json
    }

    private static func parseReplayCompletedLastSeq(_ raw: String) -> Int? {
        guard let data = raw.data(using: .utf8),
              let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let lastSeq = json["last_seq"] as? Int
        else { return nil }
        return lastSeq
    }

    private static func parseSessionEvent(_ raw: String) throws -> AgentSessionEvent {
        guard let data = raw.data(using: .utf8),
              let json = try JSONSerialization.jsonObject(with: data) as? [String: Any],
              let type = json["type"] as? String else {
            throw WaveServiceError.commandFailed("Invalid session event payload")
        }

        switch type {
        case "turn_started":
            return .turnStarted(turnId: json["turn_id"] as? String ?? "")
        case "turn_completed":
            return .turnCompleted(
                turnId: json["turn_id"] as? String ?? "",
                status: json["status"] as? String ?? "completed"
            )
        case "context_snapshot":
            guard let snapshotJSON = json["snapshot"] as? [String: Any] else {
                return .other(type: type, payload: parseJSONValue(json))
            }
            return .contextSnapshot(parseContextSnapshot(snapshotJSON))
        case "item_started":
            let turnId = json["turn_id"] as? String ?? ""
            guard let itemJSON = json["item"] as? [String: Any] else {
                return .other(type: type, payload: parseJSONValue(json))
            }
            return .itemStarted(turnId: turnId, item: parseSessionItem(itemJSON))
        case "item_updated":
            let turnId = json["turn_id"] as? String ?? ""
            let itemId = json["item_id"] as? String ?? ""
            guard let deltaJSON = json["data"] as? [String: Any] else {
                return .other(type: type, payload: parseJSONValue(json))
            }
            return .itemUpdated(turnId: turnId, itemId: itemId, delta: parseItemDelta(deltaJSON))
        case "item_completed":
            let turnId = json["turn_id"] as? String ?? ""
            guard let itemJSON = json["item"] as? [String: Any] else {
                return .other(type: type, payload: parseJSONValue(json))
            }
            return .itemCompleted(turnId: turnId, item: parseSessionItem(itemJSON))
        case "text_delta":
            return .textDelta(
                turnId: json["turn_id"] as? String ?? "",
                content: json["content"] as? String ?? ""
            )
        case "reasoning_delta":
            return .reasoningDelta(
                turnId: json["turn_id"] as? String ?? "",
                content: json["content"] as? String ?? ""
            )
        case "diff_updated":
            return .diffUpdated(
                turnId: json["turn_id"] as? String ?? "",
                diff: json["diff"] as? String ?? ""
            )
        case "suggested_actions":
            let turnId = json["turn_id"] as? String ?? ""
            let actionJSONList: [[String: Any]] = json["actions"] as? [[String: Any]] ?? []
            let actions: [SuggestedActionPayload] = actionJSONList.compactMap { actionJSON in
                guard let label = actionJSON["label"] as? String else { return nil }
                return SuggestedActionPayload(
                    label: label,
                    description: actionJSON["description"] as? String
                )
            }
            return .suggestedActions(turnId: turnId, actions: actions)
        case "status_changed":
            return .statusChanged(status: json["status"] as? String ?? "")
        case "error":
            return .error(
                code: json["code"] as? String ?? "error",
                message: json["message"] as? String ?? "Session error"
            )
        default:
            return .other(type: type, payload: parseJSONValue(json))
        }
    }

    private static func parseSessionItem(_ json: [String: Any]) -> SessionItem {
        let type = json["type"] as? String ?? "unknown"
        switch type {
        case "command":
            let item = CommandItem(
                id: json["id"] as? String ?? "",
                command: normalizeStringList(json["command"]),
                cwd: json["cwd"] as? String ?? "",
                status: parseItemStatus(json["status"]) ?? .inProgress,
                output: json["output"] as? String,
                exitCode: normalizeOptionalInt(json["exit_code"]),
                durationMs: normalizeOptionalUInt64(json["duration_ms"])
            )
            return .command(item)
        case "file":
            let changeList = (json["changes"] as? [[String: Any]] ?? []).map {
                FileEdit(
                    path: $0["path"] as? String ?? "",
                    kind: $0["kind"] as? String,
                    diff: $0["diff"] as? String
                )
            }
            let item = FileItem(
                id: json["id"] as? String ?? "",
                changes: changeList,
                status: parseItemStatus(json["status"]) ?? .inProgress
            )
            return .file(item)
        case "message":
            return .message(
                MessageItem(
                    id: json["id"] as? String ?? "",
                    text: json["text"] as? String ?? "",
                    phase: json["phase"] as? String
                )
            )
        case "thought":
            return .thought(
                ThoughtItem(
                    id: json["id"] as? String ?? "",
                    text: json["text"] as? String ?? ""
                )
            )
        case "tool":
            return .tool(
                ToolItem(
                    id: json["id"] as? String ?? "",
                    name: json["name"] as? String ?? "",
                    status: parseItemStatus(json["status"]) ?? .inProgress,
                    input: parseJSONValueOptional(json["input"]),
                    output: json["output"] as? String
                )
            )
        default:
            return .unknown(type: type, payload: parseJSONValue(json))
        }
    }

    private static func parseItemDelta(_ json: [String: Any]) -> ItemDelta {
        let type = json["type"] as? String ?? "unknown"
        switch type {
        case "output":
            return .output(content: json["content"] as? String ?? "")
        case "plan_text":
            return .planText(content: json["content"] as? String ?? "")
        default:
            return .unknown(type: type, payload: parseJSONValue(json))
        }
    }

    private static func parseItemStatus(_ raw: Any?) -> ItemStatus? {
        guard let rawString = raw as? String else { return nil }
        return ItemStatus(rawValue: rawString)
    }

    private static func parseContextSnapshot(_ json: [String: Any]) -> ContextSnapshot {
        let sources = parseStringUInt64Map(json["sources"])
        let sourceCounts = parseStringUInt64Map(json["source_counts"])
        let documents: [DocumentEntry] = (json["documents"] as? [[String: Any]] ?? []).compactMap {
            documentJSON -> DocumentEntry? in
            guard let path = documentJSON["path"] as? String,
                  let source = documentJSON["source"] as? String
            else { return nil }
            return DocumentEntry(
                path: path,
                source: source,
                tokens: normalizeUInt64(documentJSON["tokens"])
            )
        }
        return ContextSnapshot(
            sources: sources,
            sourceCounts: sourceCounts,
            documents: documents,
            budget: normalizeUInt64(json["budget"]),
            total: normalizeUInt64(json["total"]),
            diffTier: json["diff_tier"] as? String ?? "None",
            stepName: json["step_name"] as? String,
            directionNames: normalizeStringList(json["direction_names"]),
            areaName: json["area_name"] as? String,
            waveName: json["wave_name"] as? String,
            hasClipboard: json["has_clipboard"] as? Bool ?? false
        )
    }

    private static func parseStringUInt64Map(_ raw: Any?) -> [String: UInt64] {
        guard let sourceMap = raw as? [String: Any] else { return [:] }
        var parsed: [String: UInt64] = [:]
        parsed.reserveCapacity(sourceMap.count)
        for (key, value) in sourceMap {
            parsed[key] = normalizeUInt64(value)
        }
        return parsed
    }

    private static func parseJSONValueOptional(_ value: Any?) -> JSONValue? {
        guard let value else { return nil }
        return parseJSONValue(value)
    }

    private static func parseJSONValue(_ value: Any) -> JSONValue {
        if let dict = value as? [String: Any] {
            let mapped = dict.mapValues { parseJSONValue($0) }
            return .object(mapped)
        }
        if let array = value as? [Any] {
            return .array(array.map(parseJSONValue))
        }
        if let string = value as? String {
            return .string(string)
        }
        if let bool = value as? Bool {
            return .bool(bool)
        }
        if let number = value as? NSNumber {
            return .number(number.doubleValue)
        }
        if value is NSNull {
            return .null
        }
        return .string(String(describing: value))
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

    private static func normalizeOptionalInt(_ value: Any?) -> Int? {
        if let intValue = value as? Int { return intValue }
        if let doubleValue = value as? Double { return Int(doubleValue) }
        return nil
    }

    private static func normalizeOptionalUInt64(_ value: Any?) -> UInt64? {
        if let uintValue = value as? UInt64 {
            return uintValue
        }
        if let intValue = value as? Int, intValue >= 0 {
            return UInt64(intValue)
        }
        if let doubleValue = value as? Double, doubleValue >= 0 {
            return UInt64(doubleValue)
        }
        if let number = value as? NSNumber {
            let intValue = number.int64Value
            if intValue >= 0 {
                return UInt64(intValue)
            }
        }
        return nil
    }

    private static func normalizeUInt64(_ value: Any?) -> UInt64 {
        normalizeOptionalUInt64(value) ?? 0
    }

    private static func parseRemoteRepo(_ json: [String: Any]) -> RemoteRepo? {
        guard let path = json["path"] as? String else {
            return nil
        }
        let name = (json["name"] as? String).flatMap { value in
            value.isEmpty ? nil : value
        } ?? URL(fileURLWithPath: path).lastPathComponent
        let registered = json["registered"] as? Bool ?? false
        return RemoteRepo(
            path: path,
            name: name,
            waveCount: normalizeInt(json["wave_count"]),
            registered: registered,
            addedAt: parseDate(json["added_at"])
        )
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

    private static func usageQueryItems(
        filters: UsageAnalyticsFilters,
        groupBy: UsageGroupBy,
        bucket: UsageTimeBucket?
    ) -> [URLQueryItem] {
        var items: [URLQueryItem] = [URLQueryItem(name: "group_by", value: groupBy.rawValue)]
        if let bucket {
            items.append(URLQueryItem(name: "bucket", value: bucket.rawValue))
        }
        if let wave = filters.wave {
            items.append(URLQueryItem(name: "wave", value: wave))
        }
        if let flow = filters.flow {
            items.append(URLQueryItem(name: "flow", value: flow))
        }
        if let step = filters.step {
            items.append(URLQueryItem(name: "step", value: step))
        }
        if let model = filters.model {
            items.append(URLQueryItem(name: "model", value: model))
        }
        if let from = filters.from {
            items.append(URLQueryItem(name: "from", value: formatUsageDate(from)))
        }
        if let to = filters.to {
            items.append(URLQueryItem(name: "to", value: formatUsageDate(to)))
        }
        return items
    }

    private static func formatUsageDate(_ date: Date) -> String {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        return formatter.string(from: date)
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

}

public enum WaveServiceError: LocalizedError {
    case commandFailed(String)
    case authRejected(String?)
    case serverError(status: Int, message: String?)
    case daemonTimeout(String?)
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
        case .daemonTimeout(let message):
            return message ?? "Agent timed out — check server logs"
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
