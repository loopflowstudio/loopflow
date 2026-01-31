// Service for subscribing to lfd events via Unix socket.

import Foundation
import Network
import os.log

private let logger = Logger(subsystem: "com.loopflow.concerto", category: "lfd-events")

// Event types from lfd

public struct WorktreeEvent: Sendable {
    public let name: String
    public let branch: String?
    public let path: String?
    public let reason: String?
    public let repo: String?
    public let worktree: Worktree?  // Full status for in-place updates (rich events)
}

public struct SessionEvent: Sendable {
    public let id: String
    public let step: String?  // The prompt/step name (daemon sends as "task")
    public let status: String?
    public let worktree: String?
}

public struct OutputEvent: Sendable {
    public let sessionId: String
    public let text: String
    public let timestamp: Date
}

public struct WaveEvent: Sendable {
    public let name: String
    public let waveId: String?
    public let stimulus: String?
}

public enum LFDEvent: Sendable {
    case worktree(WorktreeEvent)
    case session(SessionEvent)
    case output(OutputEvent)
    case wave(WaveEvent)
}

public actor LFDEventService {
    private var connection: NWConnection?
    private let socketPath = FileManager.default.homeDirectoryForCurrentUser
        .appendingPathComponent(".lf/lfd.sock")
    private var onEvent: (@Sendable (LFDEvent) -> Void)?
    private var onConnectionChange: (@Sendable (Bool) -> Void)?
    private var patterns: [String] = []
    private var reconnectTask: Task<Void, Never>?
    private var _isConnected = false
    private var reconnectAttempts = 0

    public init() {}

    public var isConnected: Bool { _isConnected }

    public func subscribe(
        to patterns: [String],
        onEvent: @escaping @Sendable (LFDEvent) -> Void,
        onConnectionChange: @escaping @Sendable (Bool) -> Void
    ) async {
        self.patterns = patterns
        self.onEvent = onEvent
        self.onConnectionChange = onConnectionChange

        LoggingService.lfd("EventService.subscribe: patterns=\(patterns)")
        await connect()
        startReconnectLoop()
    }

    private func connect() async {
        // Check if socket exists before attempting connection
        logger.debug("connect() checking socket at: \(self.socketPath.path)")
        guard FileManager.default.fileExists(atPath: socketPath.path) else {
            logger.debug("socket does not exist")
            LoggingService.append("socket not found at \(socketPath.path)", category: LoggingService.Category.lfd)
            updateConnectionState(false)
            return
        }
        logger.debug("socket exists, creating connection")
        LoggingService.append("connecting to \(socketPath.path)", category: LoggingService.Category.lfd)

        // Unix sockets need stream-based parameters (like TCP)
        let params = NWParameters.tcp
        let endpoint = NWEndpoint.unix(path: socketPath.path)
        connection = NWConnection(to: endpoint, using: params)

        connection?.stateUpdateHandler = { [weak self] state in
            Task {
                switch state {
                case .ready:
                    await self?.handleConnected()
                case .failed, .cancelled:
                    await self?.handleDisconnected()
                default:
                    break
                }
            }
        }

        connection?.start(queue: .main)
    }

    private func handleConnected() {
        logger.info("connected to lfd")
        LoggingService.append("connected", category: LoggingService.Category.lfd)
        updateConnectionState(true)
        reconnectAttempts = 0  // Reset for next disconnect

        // Send subscribe request
        let patternsJson = patterns.map { "\"\($0)\"" }.joined(separator: ",")
        let request = "{\"method\":\"subscribe\",\"params\":{\"events\":[\(patternsJson)]}}\n"
        connection?.send(content: request.data(using: .utf8), completion: .idempotent)

        // Start receiving events
        receiveLoop()
    }

    private func handleDisconnected() {
        logger.info("disconnected from lfd")
        LoggingService.append("disconnected", category: LoggingService.Category.lfd)
        connection?.cancel()
        connection = nil
        updateConnectionState(false)
    }

    private func updateConnectionState(_ connected: Bool) {
        guard _isConnected != connected else { return }
        logger.debug("connection state changed: \(connected)")
        _isConnected = connected
        onConnectionChange?(connected)
    }

    private func startReconnectLoop() {
        reconnectTask?.cancel()
        reconnectAttempts = 0
        reconnectTask = Task {
            while !Task.isCancelled {
                // Fast reconnect at startup (0.5s for first 10 attempts), then slow down to 5s
                let delay: Duration = reconnectAttempts < 10 ? .milliseconds(500) : .seconds(5)
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
        connection?.receive(minimumIncompleteLength: 1, maximumLength: 65536) { [weak self] data, _, _, error in
            guard let self = self else { return }

            if let data = data {
                // lfd sends newline-delimited JSON
                if let text = String(data: data, encoding: .utf8) {
                    for line in text.split(separator: "\n") {
                        if let lineData = line.data(using: .utf8),
                           let json = try? JSONSerialization.jsonObject(with: lineData) as? [String: Any],
                           let eventName = json["event"] as? String,
                           let eventData = json["data"] as? [String: Any] {
                            // Parse event synchronously (nonisolated function)
                            let event = Self.parseEvent(name: eventName, data: eventData)
                            Task { await self.handleEvent(event) }
                        }
                    }
                }
            }

            if error == nil {
                Task { await self.receiveLoop() }
            }
        }
    }

    private nonisolated static func parseEvent(name: String, data: [String: Any]) -> LFDEvent {
        if name.hasPrefix("worktree.") {
            // Parse rich worktree data if present (lfd sends full status for in-place updates)
            var worktree: Worktree? = nil
            if let worktreeData = data["worktree"] as? [String: Any],
               let jsonData = try? JSONSerialization.data(withJSONObject: worktreeData),
               let worktreeJSON = try? JSONDecoder().decode(WorktreeJSON.self, from: jsonData) {
                worktree = Worktree(from: worktreeJSON)
            }

            return .worktree(WorktreeEvent(
                name: name,
                branch: data["branch"] as? String,
                path: data["path"] as? String,
                reason: data["reason"] as? String,
                repo: data["repo"] as? String,
                worktree: worktree
            ))
        }

        if name.hasPrefix("session.") {
            return .session(SessionEvent(
                id: data["id"] as? String ?? "",
                step: data["step"] as? String,
                status: data["status"] as? String,
                worktree: data["worktree"] as? String
            ))
        }

        if name == "output.line" {
            let timestamp: Date
            if let isoString = data["timestamp"] as? String {
                timestamp = ISO8601DateFormatter().date(from: isoString) ?? Date()
            } else {
                timestamp = Date()
            }
            return .output(OutputEvent(
                sessionId: data["session_id"] as? String ?? "",
                text: data["text"] as? String ?? "",
                timestamp: timestamp
            ))
        }

        if name.hasPrefix("wave.") {
            return .wave(WaveEvent(
                name: name,
                waveId: data["wave_id"] as? String ?? data["id"] as? String,
                stimulus: data["stimulus"] as? String
            ))
        }

        // Default to worktree event for unknown patterns
        return .worktree(WorktreeEvent(name: name, branch: nil, path: nil, reason: nil, repo: nil, worktree: nil))
    }

    private func handleEvent(_ event: LFDEvent) {
        onEvent?(event)
    }

    public func disconnect() {
        reconnectTask?.cancel()
        reconnectTask = nil
        connection?.cancel()
        connection = nil
        onEvent = nil
        onConnectionChange = nil
        updateConnectionState(false)
    }
}
