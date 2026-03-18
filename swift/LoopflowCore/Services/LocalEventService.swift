// Service for subscribing to lfd events via WebSocket.

import Foundation
import Network
import os.log

private let logger = Logger(subsystem: "com.loopflow.concerto", category: "lfd-events")

public struct ConnectedEvent: Sendable {
    public let timestamp: Date
    public let waves: [Wave]
}

public enum WaveEventType: String, Sendable {
    case created
    case updated
    case deleted
    case started
    case stopped
    case waiting
}

public struct WaveEvent: Sendable {
    public let type: WaveEventType
    public let waveId: String
    public let waveRunId: String?
    public let step: String?
    public let sessionId: String?
    public let initialUserMessage: String?
    public let name: String?
    public let wave: Wave?
    public let timestamp: Date
}

public struct WorktreeEvent: Sendable {
    public let worktree: String
    public let repo: String
    public let branch: String?
    public let timestamp: Date
}

public struct AgentStartedEvent: Sendable {
    public let agentId: String
    public let step: String
    public let worktree: String
    public let timestamp: Date
}

public struct AgentEndedEvent: Sendable {
    public let agentId: String
    public let status: String
    public let timestamp: Date
}

public struct AttentionEvent: Sendable {
    public enum EventType: String, Sendable {
        case created
        case updated
        case resolved
    }

    public let type: EventType
    public let item: AttentionItem?
    public let timestamp: Date
}

public struct OutputEvent: Sendable {
    public let waveId: String
    public let agentId: String
    public let text: String
    public let timestamp: Date
}

public struct AuthEvent: Sendable {
    public enum EventType: String, Sendable {
        case flowStarted = "auth.flow_started"
        case connected = "auth.connected"
        case failed = "auth.failed"
        case disconnected = "auth.disconnected"
    }

    public let type: EventType
    public let provider: AuthProvider
    public let verificationURI: String?
    public let verificationURIComplete: String?
    public let login: String?
    public let error: String?
    public let timestamp: Date
}

public enum LFDEvent: Sendable {
    case connected(ConnectedEvent)
    case wave(WaveEvent)
    case worktree(WorktreeEvent)
    case agentStarted(AgentStartedEvent)
    case agentEnded(AgentEndedEvent)
    case output(OutputEvent)
    case attention(AttentionEvent)
    case auth(AuthEvent)
    case secrets(SecretsEvent)
}

public actor EventService {
    public typealias SessionFactory = @Sendable (
        _ requestTimeout: TimeInterval,
        _ resourceTimeout: TimeInterval,
        _ delegate: URLSessionDelegate?
    ) -> URLSession

    private let connection: ServerConnection
    private let tokenProvider: @Sendable () -> String?
    private let sessionFactory: SessionFactory
    private let pinStore: CertificatePinStore
    private let pathMonitor = NWPathMonitor()

    private var webSocketTask: URLSessionWebSocketTask?
    private var onEvent: (@Sendable (LFDEvent) -> Void)?
    private var onConnectionStateChange: (@Sendable (ConnectionState) -> Void)?
    private var reconnectTask: Task<Void, Never>?
    private var reconnectAttempts = 0
    private var _isConnected = false
    private var intentionallyDisconnected = false
    private var isPathMonitorStarted = false

    private struct SocketConnection {
        let task: URLSessionWebSocketTask
        let delegate: CertificatePinningDelegate?
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

    public var isConnected: Bool { _isConnected }

    public func subscribe(
        onEvent: @escaping @Sendable (LFDEvent) -> Void,
        onConnectionStateChange: @escaping @Sendable (ConnectionState) -> Void
    ) async {
        self.onEvent = onEvent
        self.onConnectionStateChange = onConnectionStateChange
        intentionallyDisconnected = false

        startPathMonitoringIfNeeded()
        emitConnectionState(.connecting(.wsProbe))

        do {
            try await connect()
        } catch {
            handleConnectionFailure(error)
        }
    }

    public func probe(timeout: TimeInterval = 5) async throws {
        let socket = try makeWebSocketConnection()
        let task = socket.task
        task.resume()

        defer {
            task.cancel(with: .normalClosure, reason: nil)
        }

        try await withThrowingTaskGroup(of: Void.self) { group in
            group.addTask {
                while true {
                    let message = try await task.receive()
                    guard let text = Self.extractText(from: message),
                          let event = Self.parseEvent(text: text) else {
                        continue
                    }
                    if case .connected = event {
                        return
                    }
                }
            }

            group.addTask {
                try await Task.sleep(for: .seconds(timeout))
                throw WaveServiceError.timeout
            }

            _ = try await group.next()
            group.cancelAll()
        }

        if let trustError = trustMismatchError(from: socket.delegate) {
            throw trustError
        }
    }

    private func makeSession(delegate: URLSessionDelegate?) -> URLSession {
        let requestTimeout: TimeInterval = connection.isLocal ? 3 : 10
        let resourceTimeout: TimeInterval = connection.isLocal ? 10 : 30
        return sessionFactory(requestTimeout, resourceTimeout, delegate)
    }

    private func connect() async throws {
        guard webSocketTask == nil else { return }
        let socket = try makeWebSocketConnection()

        LoggingService.append("connecting to \(connection.wsBaseURL.absoluteString)", category: LoggingService.Category.lfd)

        let task = socket.task
        webSocketTask = task
        task.resume()
        receiveLoop(delegate: socket.delegate)
    }

    private func receiveLoop(delegate: CertificatePinningDelegate?) {
        webSocketTask?.receive { [weak self] result in
            guard let self else { return }
            Task {
                await self.handleReceive(result, delegate: delegate)
            }
        }
    }

    private func handleReceive(
        _ result: Result<URLSessionWebSocketTask.Message, Error>,
        delegate: CertificatePinningDelegate?
    ) async {
        switch result {
        case .failure(let error):
            if let trustState = trustRequirementState(from: delegate) {
                emitConnectionState(trustState)
            } else {
                handleConnectionFailure(error)
            }
            return

        case .success(let message):
            if let text = Self.extractText(from: message),
               let event = Self.parseEvent(text: text) {
                if case .connected = event {
                    handleConnected()
                }
                onEvent?(event)
            }
        }

        receiveLoop(delegate: delegate)
    }

    private func handleConnected() {
        reconnectAttempts = 0
        _isConnected = true
        emitConnectionState(.connected)
    }

    private func handleConnectionFailure(_ error: Error) {
        logger.info("disconnected from lfd: \(error.localizedDescription)")
        LoggingService.append("disconnected: \(error.localizedDescription)", category: LoggingService.Category.lfd)

        webSocketTask?.cancel(with: .goingAway, reason: nil)
        webSocketTask = nil
        _isConnected = false

        guard !intentionallyDisconnected else {
            emitConnectionState(.disconnected(nil))
            return
        }

        if let waveError = error as? WaveServiceError {
            switch waveError {
            case .authRejected(let message):
                emitConnectionState(.authFailed(message))
                return
            case .trustMismatch(let oldFingerprint, let newFingerprint):
                emitConnectionState(
                    .trustRequired(
                        .certificateChanged(
                            host: connection.host,
                            port: connection.port,
                            oldFingerprint: oldFingerprint,
                            newFingerprint: newFingerprint
                        )
                    )
                )
                return
            default:
                break
            }
        }

        scheduleReconnect(lastError: mapDisconnectReason(error))
    }

    private func mapDisconnectReason(_ error: Error) -> DisconnectReason {
        if let waveError = error as? WaveServiceError {
            switch waveError {
            case .authRejected:
                return .authRejected
            case .daemonTimeout:
                return .daemonTimeout
            case .timeout:
                return .timeout
            case .networkUnavailable:
                return .networkUnavailable
            case .serverUnreachable:
                return .serverUnreachable
            case .serverError(let status, _):
                return .serverError(status: status)
            case .trustMismatch:
                return .trustMismatch
            case .commandFailed(let message):
                return .unknown(message)
            }
        }

        if let urlError = error as? URLError {
            switch urlError.code {
            case .timedOut:
                return .timeout
            case .notConnectedToInternet, .networkConnectionLost:
                return .networkUnavailable
            case .cannotConnectToHost, .cannotFindHost, .dnsLookupFailed:
                return .serverUnreachable
            default:
                return .unknown(urlError.localizedDescription)
            }
        }

        return .unknown(error.localizedDescription)
    }

    private func scheduleReconnect(lastError: DisconnectReason) {
        reconnectTask?.cancel()
        reconnectAttempts += 1

        let baseDelay = min(pow(2.0, Double(max(0, reconnectAttempts - 1))), 30.0)
        let jitter = Double.random(in: 0...0.4)
        let delay = min(baseDelay + jitter, 30.0)

        emitConnectionState(
            .reconnecting(
                attempt: reconnectAttempts,
                nextDelay: delay,
                lastError: lastError
            )
        )

        reconnectTask = Task { [weak self] in
            try? await Task.sleep(for: .seconds(delay))
            guard let self, !Task.isCancelled else { return }
            await self.retryNow()
        }
    }

    private func retryNow() async {
        guard !_isConnected else { return }
        do {
            try await connect()
        } catch {
            handleConnectionFailure(error)
        }
    }

    private func startPathMonitoringIfNeeded() {
        guard !isPathMonitorStarted else { return }
        isPathMonitorStarted = true

        pathMonitor.pathUpdateHandler = { [weak self] path in
            guard path.status == .satisfied else { return }
            Task {
                await self?.resumeFromBackground()
            }
        }
        pathMonitor.start(queue: DispatchQueue.global(qos: .utility))
    }

    public func resumeFromBackground() async {
        guard !_isConnected else { return }
        reconnectTask?.cancel()
        reconnectTask = nil
        await retryNow()
    }

    private func emitConnectionState(_ state: ConnectionState) {
        onConnectionStateChange?(state)
    }

    private func makeWebSocketConnection() throws -> SocketConnection {
        let delegate = connection.useTLS
            ? CertificatePinningDelegate(connection: connection, pinStore: pinStore)
            : nil
        let session = makeSession(delegate: delegate)
        var request = URLRequest(url: connection.wsBaseURL)
        try applyAuthorization(to: &request)
        return SocketConnection(task: session.webSocketTask(with: request), delegate: delegate)
    }

    private func applyAuthorization(to request: inout URLRequest) throws {
        if connection.authMode.requiresToken {
            guard let token = tokenProvider(), !token.isEmpty else {
                throw WaveServiceError.authRejected("Missing token")
            }
            request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
            return
        }

        guard connection.isLocal,
              let token = tokenProvider(),
              !token.isEmpty else {
            return
        }

        request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
    }

    private func trustRequirementState(from delegate: CertificatePinningDelegate?) -> ConnectionState? {
        guard let requirement = delegate?.consumeTrustRequirement() else { return nil }
        return .trustRequired(requirement)
    }

    private func trustMismatchError(from delegate: CertificatePinningDelegate?) -> WaveServiceError? {
        guard let requirement = delegate?.consumeTrustRequirement(),
              case let .certificateChanged(_, _, oldFingerprint, newFingerprint) = requirement else {
            return nil
        }
        return .trustMismatch(
            oldFingerprint: oldFingerprint,
            newFingerprint: newFingerprint
        )
    }

    private nonisolated static func extractText(from message: URLSessionWebSocketTask.Message) -> String? {
        switch message {
        case .string(let text):
            return text
        case .data(let data):
            return String(data: data, encoding: .utf8)
        @unknown default:
            return nil
        }
    }

    nonisolated static func parseEvent(text: String) -> LFDEvent? {
        guard let data = text.data(using: .utf8),
              let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let type = json["type"] as? String else {
            return nil
        }

        switch type {
        case "connected":
            let timestamp = parseTimestamp(json["timestamp"])
            let wavesData = json["waves"] as? [[String: Any]] ?? []
            let waves = wavesData.map { WaveService.parseWaveFromJSON($0) }
            return .connected(ConnectedEvent(timestamp: timestamp, waves: waves))
        case "ping":
            return nil
        case "wave_created", "wave_updated", "wave_deleted",
             "wave_started", "wave_stopped", "wave_waiting":
            guard let waveId = json["wave_id"] as? String,
                  let eventType = WaveEventType(rawValue: String(type.dropFirst(5))) else { return nil }
            let wave = (json["wave"] as? [String: Any]).map { WaveService.parseWaveFromJSON($0) }
            return .wave(WaveEvent(
                type: eventType,
                waveId: waveId,
                waveRunId: json["wave_run_id"] as? String,
                step: json["step"] as? String,
                sessionId: json["session_id"] as? String,
                initialUserMessage: json["initial_user_message"] as? String,
                name: json["name"] as? String,
                wave: wave,
                timestamp: parseTimestamp(json["timestamp"])
            ))
        case "worktree_updated":
            guard let worktree = json["worktree"] as? String,
                  let repo = json["repo"] as? String else { return nil }
            return .worktree(WorktreeEvent(
                worktree: worktree,
                repo: repo,
                branch: json["branch"] as? String,
                timestamp: parseTimestamp(json["timestamp"])
            ))
        case "agent_started":
            guard let agentId = json["agent_id"] as? String,
                  let step = json["step"] as? String,
                  let worktree = json["worktree"] as? String else { return nil }
            return .agentStarted(AgentStartedEvent(
                agentId: agentId,
                step: step,
                worktree: worktree,
                timestamp: parseTimestamp(json["timestamp"])
            ))
        case "agent_ended":
            guard let agentId = json["agent_id"] as? String,
                  let status = json["status"] as? String else { return nil }
            return .agentEnded(AgentEndedEvent(
                agentId: agentId,
                status: status,
                timestamp: parseTimestamp(json["timestamp"])
            ))
        case "output_line":
            guard let waveId = json["wave_id"] as? String,
                  let agentId = json["agent_id"] as? String,
                  let text = json["text"] as? String else { return nil }
            return .output(OutputEvent(
                waveId: waveId,
                agentId: agentId,
                text: text,
                timestamp: parseTimestamp(json["timestamp"])
            ))
        case "attention_created", "attention_updated", "attention_resolved":
            let typeMap: [String: AttentionEvent.EventType] = [
                "attention_created": .created,
                "attention_updated": .updated,
                "attention_resolved": .resolved,
            ]
            guard let eventType = typeMap[type] else { return nil }
            let item = (json["item"] as? [String: Any]).flatMap(WaveService.parseAttentionFromJSON)
            return .attention(AttentionEvent(type: eventType, item: item, timestamp: parseTimestamp(json["timestamp"])))
        case "auth.flow_started", "auth.connected", "auth.failed", "auth.disconnected":
            guard let authEvent = parseAuthEvent(type: type, json: json) else {
                return nil
            }
            return .auth(authEvent)
        case "secrets.connected", "secrets.synced", "secrets.disconnected":
            guard let eventType = SecretsEvent.EventType(rawValue: type) else {
                return nil
            }
            let provider = json["provider"] as? String
            return .secrets(SecretsEvent(
                type: eventType,
                provider: provider,
                timestamp: parseTimestamp(json["timestamp"])
            ))
        default:
            return nil
        }
    }

    private nonisolated static func parseAuthEvent(type: String, json: [String: Any]) -> AuthEvent? {
        guard let eventType = AuthEvent.EventType(rawValue: type),
              let providerRaw = json["provider"] as? String,
              let provider = AuthProvider(rawValue: providerRaw) else {
            return nil
        }

        return AuthEvent(
            type: eventType,
            provider: provider,
            verificationURI: json["verification_uri"] as? String,
            verificationURIComplete: json["verification_uri_complete"] as? String,
            login: json["login"] as? String,
            error: parseErrorMessage(json["error"]),
            timestamp: parseTimestamp(json["timestamp"])
        )
    }

    private nonisolated static func parseErrorMessage(_ value: Any?) -> String? {
        if let error = value as? String {
            return error
        }
        if let errorJSON = value as? [String: Any] {
            return errorJSON["message"] as? String
        }
        return nil
    }

    private nonisolated static func parseTimestamp(_ value: Any?) -> Date {
        guard let string = value as? String else { return Date() }
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        if let date = formatter.date(from: string) {
            return date
        }
        return ISO8601DateFormatter().date(from: string) ?? Date()
    }

    public func disconnect() async {
        intentionallyDisconnected = true
        reconnectTask?.cancel()
        reconnectTask = nil
        webSocketTask?.cancel(with: .normalClosure, reason: nil)
        webSocketTask = nil
        onEvent = nil
        onConnectionStateChange = nil
        _isConnected = false
    }
}

public typealias LocalEventService = EventService
