// Service for subscribing to lfd events via WebSocket.

import Foundation
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

public struct OutputEvent: Sendable {
    public let waveId: String
    public let agentId: String
    public let text: String
    public let timestamp: Date
}

public enum LFDEvent: Sendable {
    case connected(ConnectedEvent)
    case wave(WaveEvent)
    case worktree(WorktreeEvent)
    case agentStarted(AgentStartedEvent)
    case agentEnded(AgentEndedEvent)
    case output(OutputEvent)
}

public actor LocalEventService {
    private let session: URLSession
    private let wsURL = URL(string: "ws://127.0.0.1:\(lfdDefaultPort)/ws")!
    private var webSocketTask: URLSessionWebSocketTask?
    private var onEvent: (@Sendable (LFDEvent) -> Void)?
    private var onConnectionChange: (@Sendable (Bool) -> Void)?
    private var reconnectTask: Task<Void, Never>?
    private var reconnectAttempts = 0
    private var _isConnected = false

    public init() {
        let config = URLSessionConfiguration.default
        config.timeoutIntervalForRequest = 5
        config.timeoutIntervalForResource = 10
        session = URLSession(configuration: config)
    }

    public var isConnected: Bool { _isConnected }

    public func subscribe(
        onEvent: @escaping @Sendable (LFDEvent) -> Void,
        onConnectionChange: @escaping @Sendable (Bool) -> Void
    ) async {
        self.onEvent = onEvent
        self.onConnectionChange = onConnectionChange

        LoggingService.lfd("EventService.subscribe: connecting to ws")
        await connect()
        startReconnectLoop()
    }

    private func connect() async {
        guard webSocketTask == nil else { return }
        LoggingService.append("connecting to \(wsURL.absoluteString)", category: LoggingService.Category.lfd)

        let task = session.webSocketTask(with: wsURL)
        webSocketTask = task
        task.resume()
        receiveLoop()
    }

    private func handleConnected() {
        updateConnectionState(true)
        reconnectAttempts = 0
    }

    private func handleDisconnected() {
        logger.info("disconnected from lfd")
        LoggingService.append("disconnected", category: LoggingService.Category.lfd)
        webSocketTask?.cancel(with: .goingAway, reason: nil)
        webSocketTask = nil
        updateConnectionState(false)
    }

    private func updateConnectionState(_ connected: Bool) {
        guard _isConnected != connected else { return }
        _isConnected = connected
        onConnectionChange?(connected)
    }

    private func startReconnectLoop() {
        reconnectTask?.cancel()
        reconnectAttempts = 0
        reconnectTask = Task {
            while !Task.isCancelled {
                let delay: Duration = reconnectAttempts < 10 ? .seconds(1) : .seconds(5)
                try? await Task.sleep(for: delay)
                guard !Task.isCancelled else { return }

                if !_isConnected {
                    reconnectAttempts += 1
                    await connect()
                }
            }
        }
    }

    private func receiveLoop() {
        webSocketTask?.receive { [weak self] result in
            guard let self else { return }
            Task {
                await self.handleReceive(result)
            }
        }
    }

    private func handleReceive(_ result: Result<URLSessionWebSocketTask.Message, Error>) async {
        switch result {
        case .failure:
            handleDisconnected()
            return
        case .success(let message):
            if let text = extractText(from: message),
               let event = Self.parseEvent(text: text) {
                if case .connected = event {
                    handleConnected()
                }
                onEvent?(event)
            }
        }

        receiveLoop()
    }

    private func extractText(from message: URLSessionWebSocketTask.Message) -> String? {
        switch message {
        case .string(let text):
            return text
        case .data(let data):
            return String(data: data, encoding: .utf8)
        @unknown default:
            return nil
        }
    }

    private nonisolated static func parseEvent(text: String) -> LFDEvent? {
        guard let data = text.data(using: .utf8),
              let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let type = json["type"] as? String else {
            return nil
        }

        switch type {
        case "connected":
            let timestamp = parseTimestamp(json["timestamp"])
            let wavesData = json["waves"] as? [[String: Any]] ?? []
            let waves = wavesData.map { LocalWaveService.parseWaveFromJSON($0) }
            return .connected(ConnectedEvent(timestamp: timestamp, waves: waves))
        case "ping":
            return nil
        case "wave_created":
            guard let waveId = json["wave_id"] as? String else { return nil }
            let wave = (json["wave"] as? [String: Any]).map { LocalWaveService.parseWaveFromJSON($0) }
            return .wave(WaveEvent(
                type: .created,
                waveId: waveId,
                waveRunId: nil,
                step: nil,
                name: json["name"] as? String,
                wave: wave,
                timestamp: parseTimestamp(json["timestamp"])
            ))
        case "wave_updated":
            guard let waveId = json["wave_id"] as? String else { return nil }
            let wave = (json["wave"] as? [String: Any]).map { LocalWaveService.parseWaveFromJSON($0) }
            return .wave(WaveEvent(
                type: .updated,
                waveId: waveId,
                waveRunId: nil,
                step: nil,
                name: nil,
                wave: wave,
                timestamp: parseTimestamp(json["timestamp"])
            ))
        case "wave_deleted":
            guard let waveId = json["wave_id"] as? String else { return nil }
            return .wave(WaveEvent(
                type: .deleted,
                waveId: waveId,
                waveRunId: nil,
                step: nil,
                name: nil,
                wave: nil,
                timestamp: parseTimestamp(json["timestamp"])
            ))
        case "wave_started":
            guard let waveId = json["wave_id"] as? String else { return nil }
            let wave = (json["wave"] as? [String: Any]).map { LocalWaveService.parseWaveFromJSON($0) }
            return .wave(WaveEvent(
                type: .started,
                waveId: waveId,
                waveRunId: json["wave_run_id"] as? String,
                step: nil,
                name: nil,
                wave: wave,
                timestamp: parseTimestamp(json["timestamp"])
            ))
        case "wave_stopped":
            guard let waveId = json["wave_id"] as? String else { return nil }
            let wave = (json["wave"] as? [String: Any]).map { LocalWaveService.parseWaveFromJSON($0) }
            return .wave(WaveEvent(
                type: .stopped,
                waveId: waveId,
                waveRunId: nil,
                step: nil,
                name: nil,
                wave: wave,
                timestamp: parseTimestamp(json["timestamp"])
            ))
        case "wave_waiting":
            guard let waveId = json["wave_id"] as? String else { return nil }
            let wave = (json["wave"] as? [String: Any]).map { LocalWaveService.parseWaveFromJSON($0) }
            return .wave(WaveEvent(
                type: .waiting,
                waveId: waveId,
                waveRunId: json["wave_run_id"] as? String,
                step: json["step"] as? String,
                name: nil,
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
        default:
            return nil
        }
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
        reconnectTask?.cancel()
        reconnectTask = nil
        webSocketTask?.cancel(with: .normalClosure, reason: nil)
        webSocketTask = nil
        onEvent = nil
        onConnectionChange = nil
        updateConnectionState(false)
    }
}
