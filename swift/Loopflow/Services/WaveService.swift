// WaveService - command/action facade for the lf-era UI.

import Foundation

public struct WaveConfigUpdate: Sendable {
    public var name: String?
    public var area: [String]?
    public var direction: [String]?
    public var goal: String?
    public var status: WaveStatus?
    public var agent: String?
    public var skillAgents: [String: String]?

    public init(
        name: String? = nil,
        area: [String]? = nil,
        direction: [String]? = nil,
        goal: String? = nil,
        status: WaveStatus? = nil,
        agent: String? = nil,
        skillAgents: [String: String]? = nil
    ) {
        self.name = name
        self.area = area
        self.direction = direction
        self.goal = goal
        self.status = status
        self.agent = agent
        self.skillAgents = skillAgents
    }
}

public struct WaveFlowsResult: Sendable {
    public var flows: [Flow]
    public var directions: [String]

    public init(flows: [Flow], directions: [String]) {
        self.flows = flows
        self.directions = directions
    }
}

// SAFETY: WaveService is a value type with immutable captured dependencies.
public struct WaveService: @unchecked Sendable {
    public typealias ShellCommandRunner = @Sendable (_ args: [String]) async throws -> Void

    private let connection: ServerConnection
    private let shellCommandRunner: ShellCommandRunner?

    public init(
        connection: ServerConnection = .local,
        tokenProvider: (@Sendable () -> String?)? = nil,
        pinStore: CertificatePinStore = .shared,
        shellCommandRunner: ShellCommandRunner? = nil
    ) {
        self.connection = connection
        self.shellCommandRunner = shellCommandRunner
        _ = tokenProvider
        _ = pinStore
    }

    public func fetchCatalog(repo: String? = nil) async throws -> Catalog {
        _ = repo
        return Catalog(flows: [], skills: [])
    }

    public func listAuthProviders() async throws -> [AuthProviderStatus] {
        []
    }

    public func startAuthFlow(provider: AuthProvider) async throws -> AuthFlow {
        throw unsupported("Provider auth is no longer read through lfd HTTP; run `lf auth \(provider.rawValue)`.")
    }

    public func disconnectProvider(provider: AuthProvider) async throws -> AuthProviderStatus {
        throw unsupported("Provider auth is no longer mutated through lfd HTTP; run `lf auth disconnect \(provider.rawValue)`.")
    }

    public func listWaves(repo: RepoTarget) async throws -> [Wave] {
        _ = repo
        throw unsupported("Wave discovery must use RegistryQuery (`lf ls --json`).")
    }

    public func getWave(_ id: String) async throws -> Wave {
        _ = id
        throw unsupported("Wave lookup must use RegistryQuery (`lf status --json`).")
    }

    public func updateWave(_ id: String, config: WaveConfigUpdate) async throws -> Wave {
        _ = id
        _ = config
        throw unsupported("Wave updates are not available through the retired lfd HTTP API.")
    }

    public func deleteWave(_ id: String) async throws {
        _ = id
        throw unsupported("Wave deletion is not available through the retired lfd HTTP API.")
    }

    public func addTrigger(_ waveId: String, signal: Trigger.Signal, flow: String? = nil) async throws -> Trigger {
        _ = waveId
        _ = signal
        _ = flow
        throw unsupported("Trigger mutation is not available through the retired lfd HTTP API.")
    }

    public func removeTrigger(_ waveId: String, triggerId: String) async throws {
        _ = waveId
        _ = triggerId
        throw unsupported("Trigger mutation is not available through the retired lfd HTTP API.")
    }

    public func stop(_ id: String) async throws {
        _ = id
        throw unsupported("Wave stop is not available through the retired lfd HTTP API.")
    }

    public func listAttention(repo: RepoTarget) async throws -> [AttentionItem] {
        _ = repo
        throw unsupported("Attention reads must use RegistryQuery (`lf status --json`).")
    }

    public func getAttention(_ id: String) async throws -> AttentionItem {
        _ = id
        throw unsupported("Attention lookup must use RegistryQuery (`lf status --json`).")
    }

    public func markAttentionViewed(_ id: String) async throws -> AttentionItem {
        _ = id
        throw unsupported("Attention mutation is not available through the retired lfd HTTP API.")
    }

    public func listFlowsAndDirections(repo: RepoTarget) async throws -> WaveFlowsResult {
        guard case .local(let url) = repo else {
            return WaveFlowsResult(flows: [], directions: [])
        }
        let lfDir = url.appendingPathComponent(".lf", isDirectory: true)
        return WaveFlowsResult(
            flows: Self.listNamedMarkdown(in: lfDir.appendingPathComponent("flows", isDirectory: true), type: .flow)
                + Self.listNamedMarkdown(in: lfDir.appendingPathComponent("skills", isDirectory: true), type: .skill),
            directions: Self.listNames(in: lfDir.appendingPathComponent("directions", isDirectory: true))
        )
    }

    public func listWorktrees(repo: RepoTarget) async throws -> [WorktreeInfo] {
        _ = repo
        return []
    }

    public func listRepos() async throws -> [RemoteRepo] {
        []
    }

    public func addRepo(path: String) async throws -> RemoteRepo {
        _ = path
        throw unsupported("Remote repo mutation is not available through the retired lfd HTTP API.")
    }

    public func removeRepo(path: String) async throws {
        _ = path
        throw unsupported("Remote repo mutation is not available through the retired lfd HTTP API.")
    }

    public func checkConnection() async throws {
        _ = connection
    }

    public func connectLfd() async throws {
        guard let shellCommandRunner else { return }
        try await shellCommandRunner(["lfd", "install"])
    }

    public func streamOutput(waveId: String) -> AsyncThrowingStream<String, Error> {
        _ = waveId
        return AsyncThrowingStream { continuation in
            continuation.finish()
        }
    }

    private func unsupported(_ message: String) -> WaveServiceError {
        WaveServiceError.commandFailed(message)
    }

    private static func listNamedMarkdown(in directory: URL, type: FlowType) -> [Flow] {
        listNames(in: directory).map { name in
            Flow(name: name, skills: [Skill(prompt: name)], type: type)
        }
    }

    private static func listNames(in directory: URL) -> [String] {
        guard let entries = try? FileManager.default.contentsOfDirectory(
            at: directory,
            includingPropertiesForKeys: [.isRegularFileKey],
            options: [.skipsHiddenFiles]
        ) else {
            return []
        }
        return entries.compactMap { entry in
            guard entry.pathExtension == "md" else { return nil }
            return entry.deletingPathExtension().lastPathComponent
        }
        .sorted()
    }
}

public extension WaveService {
    static func parseAttentionFromJSON(_ json: [String: Any]) -> AttentionItem? {
        guard let id = json["id"] as? String,
              let waveId = json["wave_id"] as? String,
              let kindRaw = json["kind"] as? String,
              let kind = AttentionKind(rawValue: kindRaw),
              let statusRaw = json["status"] as? String,
              let status = AttentionStatus(rawValue: statusRaw),
              let title = json["title"] as? String,
              let summary = json["summary"] as? String
        else {
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
            context: AttentionItem.context(kind: kind, json: json["context"] as? [String: Any] ?? json),
            surfacedAt: parseDate(json["surfaced_at"]) ?? Date.distantPast,
            viewedAt: parseDate(json["viewed_at"]),
            resolvedAt: parseDate(json["resolved_at"])
        )
    }

    static func parseWaveFromJSON(_ json: [String: Any]) -> Wave {
        let statusValue = json["status"] as? String ?? "idle"
        let normalizedStatus = switch statusValue {
        case "error": "failed"
        case "completed": "idle"
        default: statusValue
        }

        return Wave(
            id: json["id"] as? String ?? UUID().uuidString,
            name: json["name"] as? String ?? "",
            repo: json["repo"] as? String ?? "",
            goal: json["goal"] as? String ?? "",
            metrics: json["metrics"] as? [String] ?? [],
            direction: normalizeStringList(json["direction"]),
            area: normalizeStringList(json["area"]),
            agent: json["agent"] as? String,
            skillAgents: normalizeStringMap(json["skill_agents"]),
            triggers: parseTriggers(json["triggers"]),
            crons: parseCrons(json["crons"]),
            status: WaveStatus(rawValue: normalizedStatus) ?? .idle,
            iteration: normalizeInt(json["iteration"]),
            createdAt: parseDate(json["created_at"]),
            parentWaveId: json["parent_wave_id"] as? String
        )
    }

    static func parseSessionFromJSON(_ json: [String: Any]) -> Session? {
        guard let id = json["id"] as? String,
              let waveId = json["wave_id"] as? String,
              let skill = json["skill"] as? String,
              let useRaw = json["use"] as? String,
              let sessionUse = SessionUse(rawValue: useRaw),
              let agent = json["agent"] as? String,
              let cwd = json["cwd"] as? String,
              let argv = json["argv"] as? [String],
              let env = json["env"] as? [String: String],
              let source = json["source"] as? String,
              let tmuxName = json["tmux_name"] as? String,
              let statusRaw = json["status"] as? String,
              let status = SessionStatus(rawValue: statusRaw),
              let createdAt = parseDate(json["created_at"])
        else {
            return nil
        }

        return Session(
            id: id,
            waveId: waveId,
            runId: json["run_id"] as? String,
            parentSessionId: json["parent_session_id"] as? String,
            sessionUse: sessionUse,
            skill: skill,
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

    private static func parseTriggers(_ value: Any?) -> [Trigger] {
        guard let items = value as? [[String: Any]] else { return [] }
        return items.compactMap { item in
            guard let id = item["id"] as? String,
                  let signalRaw = item["signal"] as? String,
                  let signal = Trigger.Signal(rawValue: signalRaw)
            else {
                return nil
            }
            return Trigger(
                id: id,
                signal: signal,
                flow: item["flow"] as? String,
                sourceWaveId: item["source_wave_id"] as? String
            )
        }
    }

    private static func parseCrons(_ value: Any?) -> [WaveCron] {
        guard let items = value as? [[String: Any]] else { return [] }
        return items.compactMap { item in
            guard let id = item["id"] as? String,
                  let flow = item["flow"] as? String,
                  let schedule = item["schedule"] as? String
            else {
                return nil
            }
            return WaveCron(
                id: id,
                flow: flow,
                schedule: schedule,
                lastTriggeredAt: normalizeUnixDate(item["last_triggered_at"]),
                createdAt: parseDate(item["created_at"])
            )
        }
    }

    private static func normalizeStringList(_ value: Any?) -> [String] {
        if let list = value as? [String] { return list }
        if let string = value as? String { return decodeStringArray(string) }
        return []
    }

    private static func normalizeStringMap(_ value: Any?) -> [String: String]? {
        (value as? [String: Any])?.reduce(into: [String: String]()) { partial, entry in
            if let value = entry.value as? String {
                partial[entry.key] = value
            }
        }
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

    private static func parseDate(_ value: Any?) -> Date? {
        guard let dateStr = value as? String else { return nil }
        let fractional = ISO8601DateFormatter()
        fractional.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        return fractional.date(from: dateStr) ?? ISO8601DateFormatter().date(from: dateStr)
    }

    private static func decodeStringArray(_ string: String?) -> [String] {
        guard let string, !string.isEmpty else { return [] }
        guard let data = string.data(using: .utf8) else { return [string] }
        if let decoded = try? JSONDecoder().decode([String].self, from: data) {
            return decoded
        }
        if let decoded = try? JSONDecoder().decode(String.self, from: data) {
            return [decoded]
        }
        return [string]
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
            return message ?? "Agent timed out - check server logs"
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
