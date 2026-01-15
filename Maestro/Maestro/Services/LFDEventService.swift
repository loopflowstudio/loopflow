// Service for subscribing to lfd events via Unix socket.

import Foundation
import Network

// Event types from lfd

struct WorktreeEvent: Sendable {
    let name: String
    let branch: String?
    let path: String?
}

struct SessionEvent: Sendable {
    let id: String
    let task: String?
    let status: String?
}

struct OutputEvent: Sendable {
    let sessionId: String
    let text: String
    let timestamp: Date
}

enum LFDEvent: Sendable {
    case worktree(WorktreeEvent)
    case session(SessionEvent)
    case output(OutputEvent)
}

actor LFDEventService {
    private var connection: NWConnection?
    private let socketPath = FileManager.default.homeDirectoryForCurrentUser
        .appendingPathComponent(".lf/lfd.sock")
    private var onEvent: (@Sendable (LFDEvent) -> Void)?

    func subscribe(
        to patterns: [String],
        onEvent: @escaping @Sendable (LFDEvent) -> Void
    ) async throws {
        self.onEvent = onEvent

        let params = NWParameters()
        let endpoint = NWEndpoint.unix(path: socketPath.path)
        connection = NWConnection(to: endpoint, using: params)

        connection?.stateUpdateHandler = { [weak self] state in
            if case .failed = state {
                Task { await self?.disconnect() }
            }
        }

        connection?.start(queue: .main)

        // Wait briefly for connection to establish
        try await Task.sleep(for: .milliseconds(100))

        // Send subscribe request
        let patternsJson = patterns.map { "\"\($0)\"" }.joined(separator: ",")
        let request = "{\"method\":\"subscribe\",\"params\":{\"events\":[\(patternsJson)]}}\n"
        connection?.send(content: request.data(using: .utf8), completion: .idempotent)

        // Start receiving events
        receiveLoop()
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
            return .worktree(WorktreeEvent(
                name: name,
                branch: data["branch"] as? String,
                path: data["path"] as? String
            ))
        }

        if name.hasPrefix("session.") {
            return .session(SessionEvent(
                id: data["id"] as? String ?? "",
                task: data["task"] as? String,
                status: data["status"] as? String
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

        // Default to worktree event for unknown patterns
        return .worktree(WorktreeEvent(name: name, branch: nil, path: nil))
    }

    private func handleEvent(_ event: LFDEvent) {
        onEvent?(event)
    }

    func disconnect() {
        connection?.cancel()
        connection = nil
        onEvent = nil
    }
}
