// Service for loading Wave and Run data from lfd daemon (HTTP).

import Foundation

public struct WaveConfigUpdate: Sendable {
    public var name: String?
    public var area: [String]?
    public var direction: [String]?
    public var flow: String?
    public var goal: String?
    public var status: WaveStatus?
    public var agent: String?
    public var stepAgents: [String: String]?

    public init(
        name: String? = nil,
        area: [String]? = nil,
        direction: [String]? = nil,
        flow: String? = nil,
        goal: String? = nil,
        status: WaveStatus? = nil,
        agent: String? = nil,
        stepAgents: [String: String]? = nil
    ) {
        self.name = name
        self.area = area
        self.direction = direction
        self.flow = flow
        self.goal = goal
        self.status = status
        self.agent = agent
        self.stepAgents = stepAgents
    }
}

public struct RunOverrides: Sendable {
    public var area: [String]?
    public var direction: [String]?
    public var flow: String?
    public var goal: String?
    public var roadmapItem: String?

    public init(
        area: [String]? = nil,
        direction: [String]? = nil,
        flow: String? = nil,
        goal: String? = nil,
        roadmapItem: String? = nil
    ) {
        self.area = area
        self.direction = direction
        self.flow = flow
        self.goal = goal
        self.roadmapItem = roadmapItem
    }
}

public struct ConnectionInfo: Sendable {
    public let worktree: String
    public let step: String
    public let agentId: String
    public let promptFile: String
    public let runId: String?
    public let stepIndex: Int

    public init(
        worktree: String,
        step: String,
        agentId: String,
        promptFile: String,
        runId: String?,
        stepIndex: Int
    ) {
        self.worktree = worktree
        self.step = step
        self.agentId = agentId
        self.promptFile = promptFile
        self.runId = runId
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

        // Only advertise a Content-Type when we actually send a body. axum's
        // `Option<Json<T>>` extractor rejects an empty body that carries a JSON
        // content-type (EOF while parsing), which surfaced as a 400 "Terminal
        // unavailable" when starting a wave's /goal agent with no overrides.
        if let contentType, body != nil {
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

    private func jsonObject(from data: Data) throws -> [String: Any] {
        guard let json = try JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            throw WaveServiceError.commandFailed("Invalid response")
        }
        return json
    }

    private func parsedObject<T>(
        from data: Data,
        parse: ([String: Any]) -> T?
    ) throws -> T {
        let json = try jsonObject(from: data)
        guard let value = parse(json) else {
            throw WaveServiceError.commandFailed("Invalid response")
        }
        return value
    }

    private func parsedList<T>(
        from data: Data,
        parse: ([String: Any]) -> T?
    ) throws -> [T] {
        let json = try jsonObject(from: data)
        guard let items = json["data"] as? [[String: Any]] else {
            throw WaveServiceError.commandFailed("Invalid response")
        }
        return items.compactMap(parse)
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

    // MARK: - Catalog

    /// Fetch the flow + step catalog from `/v0/catalog`. Used by the Flows
    /// view to render the builtin catalog and compute "used by" parents.
    public func fetchCatalog(repo: String? = nil) async throws -> Catalog {
        var components = URLComponents(
            url: apiBaseURL.appendingPathComponent("catalog"),
            resolvingAgainstBaseURL: false
        )!
        if let repo, !repo.isEmpty {
            components.queryItems = [URLQueryItem(name: "repo", value: repo)]
        }
        let (data, response) = try await performGet(components.url!)
        try requireOKStatus(response, data: data)
        let envelope = try decodeResponse(CatalogResponse.self, from: data)
        return envelope.result
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

    // MARK: - Secrets

    public func secretsStatus() async throws -> SecretsProviderStatus {
        let (data, response) = try await performGet(secretsURL())
        try requireOKStatus(response, data: data)
        return try decodeResponse(SecretsProviderStatus.self, from: data)
    }

    public func listSecretsProjects() async throws -> [DopplerProject] {
        let (data, response) = try await performGet(secretsURL("projects"))
        try requireOKStatus(response, data: data)
        return try decodeResponse([DopplerProject].self, from: data)
    }

    public func listSecretsConfigs(project: String) async throws -> [DopplerConfig] {
        var components = URLComponents(url: secretsURL("configs"), resolvingAgainstBaseURL: false)!
        components.queryItems = [URLQueryItem(name: "project", value: project)]
        let url = components.url!
        let (data, response) = try await performGet(url)
        try requireOKStatus(response, data: data)
        return try decodeResponse([DopplerConfig].self, from: data)
    }

    public func selectSecretsConfig(
        project: String, config: String
    ) async throws -> SecretsProviderStatus {
        var request = try makeRequest(secretsURL("select"), method: "POST")
        let body = ["project": project, "config": config]
        request.httpBody = try JSONSerialization.data(withJSONObject: body)
        let (data, response) = try await performRequest(request)
        try requireOKStatus(response, data: data)
        return try decodeResponse(SecretsProviderStatus.self, from: data)
    }

    public func syncSecrets() async throws -> SecretsProviderStatus {
        let request = try makeRequest(secretsURL("sync"), method: "POST")
        let (data, response) = try await performRequest(request)
        try requireOKStatus(response, data: data)
        return try decodeResponse(SecretsProviderStatus.self, from: data)
    }

    public func disconnectSecrets() async throws -> SecretsProviderStatus {
        let request = try makeRequest(secretsURL(), method: "DELETE")
        let (data, response) = try await performRequest(request)
        try requireOKStatus(response, data: data)
        return try decodeResponse(SecretsProviderStatus.self, from: data)
    }

    private func secretsURL(_ component: String? = nil) -> URL {
        var url = apiBaseURL.appendingPathComponent("secrets")
        if let component {
            url = url.appendingPathComponent(component)
        }
        return url
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

    // MARK: - Runs

    public func listRuns(waveId: String? = nil, repo: URL? = nil, limit: Int = 50) async throws -> [Run] {
        var components = URLComponents(url: apiBaseURL.appendingPathComponent("runs"), resolvingAgainstBaseURL: false)!
        var queryItems: [URLQueryItem] = [URLQueryItem(name: "limit", value: String(limit))]
        if let waveId {
            queryItems.append(URLQueryItem(name: "wave_id", value: waveId))
        }
        if let repo {
            queryItems.append(URLQueryItem(name: "repo", value: repo.path))
        }
        components.queryItems = queryItems

        guard let url = components.url else {
            LoggingService.lfd("listRuns: invalid URL")
            return []
        }

        LoggingService.lfd("listRuns: GET \(url)")

        do {
            let (data, response) = try await performGet(url)

            guard response.statusCode == 200 else {
                throw parseStatusCodeError(statusCode: response.statusCode, data: data)
            }

            guard let json = try JSONSerialization.jsonObject(with: data) as? [String: Any],
                  let runsData = json["data"] as? [[String: Any]] else {
                LoggingService.lfd("listRuns: invalid JSON response")
                return []
            }

            return runsData.compactMap { Self.parseRunFromJSON($0) }
        } catch {
            LoggingService.lfd("listRuns: error=\(error.localizedDescription)")
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
        return try parsedList(from: data, parse: Self.parseAttentionFromJSON)
    }

    public func getAttention(_ id: String) async throws -> AttentionItem {
        let (data, response) = try await performGet(attentionURL(id))
        try requireOKStatus(response, data: data)
        return try parsedObject(from: data, parse: Self.parseAttentionFromJSON)
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
        return try parsedObject(from: data, parse: Self.parseAttentionFromJSON)
    }

    public func listSessions(
        repo: RepoTarget,
        activeOnly: Bool = true
    ) async throws -> [Session] {
        var components = URLComponents(url: sessionURL(), resolvingAgainstBaseURL: false)!
        components.queryItems = [
            URLQueryItem(name: "repo", value: repo.path),
            URLQueryItem(name: "active_only", value: activeOnly ? "true" : "false"),
        ]
        guard let url = components.url else {
            throw WaveServiceError.commandFailed("Invalid terminal sessions URL")
        }
        let (data, response) = try await performGet(url)
        try requireOKStatus(response, data: data)
        return try parsedList(from: data, parse: Self.parseSessionFromJSON)
    }

    public func createSession(
        waveId: String,
        flow: String,
        worktree: String,
        agent: String
    ) async throws -> SessionLaunchResponse {
        let request = try makeRequest(
            sessionURL(),
            method: "POST",
            body: [
                "wave_id": waveId,
                "flow": flow,
                "worktree": worktree,
                "agent": agent,
            ],
            contentType: "application/json"
        )
        let (data, response) = try await performRequest(request)
        try requireOKStatus(response, data: data)
        return try parsedObject(from: data, parse: Self.parseSessionLaunchResponseFromJSON)
    }

    public func getSession(_ id: String) async throws -> Session {
        let (data, response) = try await performGet(sessionURL(id))
        try requireOKStatus(response, data: data)
        return try parsedObject(from: data, parse: Self.parseSessionFromJSON)
    }

    public func attachSession(_ id: String) async throws -> SessionConnectionInfo {
        try await performSessionAction(
            id,
            action: "attach",
            parse: Self.parseSessionConnectionInfoFromJSON
        )
    }

    public func startSession(_ id: String) async throws -> Session {
        try await performSessionAction(
            id,
            action: "start",
            parse: Self.parseSessionFromJSON
        )
    }

    public func cancelSession(_ id: String) async throws -> Session {
        try await performSessionAction(
            id,
            action: "cancel",
            parse: Self.parseSessionFromJSON
        )
    }

    private func performSessionAction<T>(
        _ id: String,
        action: String,
        parse: ([String: Any]) -> T?
    ) async throws -> T {
        let request = try makeRequest(
            sessionURL(id, components: action),
            method: "POST",
            body: [:],
            contentType: "application/json"
        )
        let (data, response) = try await performRequest(request)
        try requireOKStatus(response, data: data)
        return try parsedObject(from: data, parse: parse)
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
        try await createWave(name: name, repo: repo, flow: "ship-roadmap", run: false, status: nil)
    }

    public func createWave(name: String, repo: RepoTarget, flow: String, run: Bool) async throws -> Wave {
        try await createWave(name: name, repo: repo, flow: flow, run: run, status: nil)
    }

    public func createWave(
        name: String,
        repo: RepoTarget,
        flow: String,
        run: Bool,
        status: WaveStatus?
    ) async throws -> Wave {
        LoggingService.lfd(
            "createWave: name=\(name.isEmpty ? "(auto)" : name) repo=\(repo.path) flow=\(flow) run=\(run) status=\(status?.rawValue ?? "default")"
        )

        var body: [String: Any] = [
            "repo": repo.path,
            "name": name.isEmpty ? NSNull() : name,
            "flow": flow,
            "direction": [],
            "run": run,
        ]
        if let status {
            body["status"] = status.rawValue
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

        let crons: [WaveCron]
        if let cronArr = json["crons"] as? [[String: Any]] {
            crons = cronArr.compactMap { dict in
                guard let id = dict["id"] as? String,
                      let flow = dict["flow"] as? String,
                      let schedule = dict["schedule"] as? String else { return nil }
                return WaveCron(
                    id: id,
                    flow: flow,
                    schedule: schedule,
                    lastTriggeredAt: normalizeUnixDate(dict["last_triggered_at"]),
                    createdAt: parseDate(dict["created_at"])
                )
            }
        } else {
            crons = []
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

        let repos: [RepoWork]
        if let reposData = json["repos"] as? [[String: Any]] {
            repos = reposData.map { Self.parseRepoWorkFromJSON($0) }
        } else {
            repos = []
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
            repos: repos,
            flow: json["primary_flow"] as? String ?? "",
            goal: json["goal"] as! String,
            metrics: json["metrics"] as! [String],
            direction: normalizeStringList(json["direction"]),
            area: normalizeStringList(json["area"]),
            agent: json["agent"] as? String,
            stepAgents: stepAgents,
            triggers: triggers,
            crons: crons,
            status: status,
            flowSteps: flowSteps,
            createdAt: createdAt
        )
    }

    /// Parse one entry of a wave's nested `repos` array into a `RepoWork`,
    /// mirroring `RepoWorkDto`.
    static func parseRepoWorkFromJSON(_ json: [String: Any]) -> RepoWork {
        let statusValue = json["status"] as? String ?? "idle"
        let normalizedStatus: String
        switch statusValue {
        case "error": normalizedStatus = "failed"
        case "completed": normalizedStatus = "idle"
        default: normalizedStatus = statusValue
        }
        let status = WaveStatus(rawValue: normalizedStatus) ?? .idle

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

        let activeRun = (json["active_run"] as? [String: Any]).flatMap(Self.parseRunFromJSON)

        let pr: PullRequest?
        if let prData = json["pr"] as? [String: Any],
           let urlString = prData["url"] as? String,
           let url = URL(string: urlString) {
            let number = prData["number"] as? Int
            let state = (prData["state"] as? String).map { $0.lowercased() }.flatMap { PRState(rawValue: $0) }
            pr = PullRequest(
                url: url,
                number: number,
                state: state,
                title: prData["title"] as? String,
                branch: prData["branch"] as? String
            )
        } else {
            pr = nil
        }

        return RepoWork(
            repo: json["repo"] as? String ?? "",
            status: status,
            iteration: normalizeInt(json["iteration"]),
            localWorktree: json["local_worktree"] as? String,
            remoteBranch: json["remote_branch"] as? String,
            commits: commits,
            diffStat: json["diff_stat"] as? String,
            openPRCount: normalizeInt(json["open_pr_count"]),
            stackCount: normalizeInt(json["stack_count"]),
            activeRun: activeRun,
            pr: pr
        )
    }

    /// Update wave configuration (name, area, direction, flow, status, agent).
    public func updateWave(_ id: String, config: WaveConfigUpdate) async throws -> Wave {
        var body: [String: Any] = [:]
        if let name = config.name { body["name"] = name }
        if let area = config.area { body["area"] = area }
        if let direction = config.direction { body["direction"] = direction }
        if let flow = config.flow { body["flow"] = flow }
        if let goal = config.goal { body["goal"] = goal }
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

    public func getWaveAgentTree(waveId: String, activeOnly: Bool = true) async throws -> WaveAgentTree {
        var components = URLComponents(
            url: apiBaseURL.appendingPathComponent("waves/\(waveId)/agent-tree"),
            resolvingAgainstBaseURL: false
        )!
        components.queryItems = [
            URLQueryItem(name: "active_only", value: activeOnly ? "true" : "false"),
        ]
        guard let url = components.url else {
            throw WaveServiceError.commandFailed("Invalid wave agent tree URL")
        }
        let (data, response) = try await performGet(url)
        try requireOKStatus(response, data: data)
        return try parsedObject(from: data, parse: Self.parseWaveAgentTreeFromJSON)
    }

    public func ensureWaveAgent(waveId id: String, overrides: RunOverrides? = nil) async throws -> Session {
        // Build body with any overrides
        var body: [String: Any] = [:]
        if let overrides {
            if let area = overrides.area { body["area"] = area }
            if let direction = overrides.direction { body["direction"] = direction }
            if let flow = overrides.flow { body["flow"] = flow }
            if let goal = overrides.goal { body["goal"] = goal }
            if let roadmapItem = overrides.roadmapItem { body["roadmap_item"] = roadmapItem }
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

        return try parsedObject(from: data, parse: Self.parseSessionFromJSON)
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
        runId: String?,
        config: AgentSessionConfig
    ) async throws -> AgentSession {
        let url = sessionURL()
        var payload = Self.sessionConfigJSON(config)
        payload["harness"] = harness
        if let runId {
            payload["run_id"] = runId
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

        let runId = json["run_id"] as? String
        let stepIndex = json["step_index"] as? Int ?? 0

        return ConnectionInfo(
            worktree: worktree,
            step: step,
            agentId: agentId,
            promptFile: promptFile,
            runId: runId,
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
              let session = Self.parseAgentSessionFromJSON(json) else {
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

    static func parseAgentSessionFromJSON(_ json: [String: Any]) -> AgentSession? {
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
            runId: json["run_id"] as? String,
            providerSessionId: json["provider_session_id"] as? String,
            inputSupported: json["input_supported"] as? Bool ?? false,
            config: config,
            createdAt: parseDate(json["created_at"]),
            endedAt: parseDate(json["ended_at"])
        )
    }

    static func parseSessionFromJSON(_ json: [String: Any]) -> Session? {
        guard let id = json["id"] as? String,
              let waveId = json["wave_id"] as? String,
              let step = json["step"] as? String,
              let sessionUseRaw = json["use"] as? String,
              let sessionUse = SessionUse(rawValue: sessionUseRaw),
              let agent = json["agent"] as? String,
              let cwd = json["cwd"] as? String,
              let argv = json["argv"] as? [String],
              let env = json["env"] as? [String: String],
              let source = json["source"] as? String,
              let tmuxName = json["tmux_name"] as? String,
              let statusRaw = json["status"] as? String,
              let status = SessionStatus(rawValue: statusRaw),
              let createdAt = parseDate(json["created_at"]) else {
            return nil
        }

        return Session(
            id: id,
            waveId: waveId,
            runId: json["run_id"] as? String,
            parentSessionId: json["parent_session_id"] as? String,
            sessionUse: sessionUse,
            step: step,
            agent: agent,
            cwd: cwd,
            argv: argv,
            env: env,
            source: source,
            tmuxName: tmuxName,
            status: status,
            createdAt: createdAt,
            attachedAt: parseDate(json["attached_at"]),
            startedAt: parseDate(json["started_at"]),
            completedAt: parseDate(json["completed_at"])
        )
    }

    static func parseSessionLaunchResponseFromJSON(_ json: [String: Any]) -> SessionLaunchResponse? {
        guard
            let sessionJSON = json["session"] as? [String: Any],
            let connectionJSON = json["connection"] as? [String: Any],
            let session = parseSessionFromJSON(sessionJSON),
            let connection = parseSessionConnectionInfoFromJSON(connectionJSON)
        else {
            return nil
        }

        return SessionLaunchResponse(session: session, connection: connection)
    }

    static func parseWaveAgentTreeFromJSON(_ json: [String: Any]) -> WaveAgentTree? {
        guard
            let object = json["object"] as? String,
            let id = json["id"] as? String,
            let waveJSON = json["wave"] as? [String: Any],
            let childWaveJSON = json["child_waves"] as? [[String: Any]],
            let sessionJSON = json["sessions"] as? [[String: Any]]
        else {
            return nil
        }

        let wave = parseWaveFromJSON(waveJSON)
        let childWaves = childWaveJSON.map(parseWaveFromJSON)
        let sessions = sessionJSON.compactMap(parseWaveAgentTreeSessionFromJSON)
        guard sessions.count == sessionJSON.count else {
            return nil
        }

        return WaveAgentTree(
            object: object,
            id: id,
            wave: wave,
            childWaves: childWaves,
            sessions: sessions
        )
    }

    static func parseWaveAgentTreeSessionFromJSON(_ json: [String: Any]) -> WaveAgentTreeSession? {
        guard
            let sessionJSON = json["session"] as? [String: Any],
            let session = parseSessionFromJSON(sessionJSON)
        else {
            return nil
        }
        let connection = (json["connection"] as? [String: Any])
            .flatMap(parseSessionConnectionInfoFromJSON)
        return WaveAgentTreeSession(session: session, connection: connection)
    }

    static func parseSessionConnectionInfoFromJSON(_ json: [String: Any]) -> SessionConnectionInfo? {
        guard
            let kind = json["kind"] as? String,
            let sessionName = json["session_name"] as? String,
            let host = json["host"] as? String,
            let cwd = json["cwd"] as? String,
            let statusRaw = json["status"] as? String,
            let status = SessionStatus(rawValue: statusRaw)
        else {
            return nil
        }

        return SessionConnectionInfo(
            kind: kind,
            sessionName: sessionName,
            host: host,
            cwd: cwd,
            status: status
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

    private static func parseRunFromJSON(_ json: [String: Any]) -> Run? {
        guard let id = json["id"] as? String,
              let flow = json["flow"] as? String,
              let repoPath = json["repo"] as? String else {
            return nil
        }

        let waveId = json["wave_id"] as? String
        let area = normalizeAreaDisplay(json["area"])
        let direction = normalizeDirection(json["direction"])
        let statusStr = json["status"] as? String ?? RunStatus.pending.rawValue
        let status = RunStatus(rawValue: statusStr) ?? .pending
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

        return Run(
            id: id,
            waveId: waveId,
            flow: flow,
            task: json["task"] as? String,
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

    private static func normalizeUnixDate(_ value: Any?) -> Date? {
        guard let seconds = normalizeOptionalInt(value) else { return nil }
        return Date(timeIntervalSince1970: TimeInterval(seconds))
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
