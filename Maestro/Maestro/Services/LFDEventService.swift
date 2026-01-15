// Service for subscribing to lfd events via Unix socket.

import Foundation
import Network

struct WorktreeEvent: Sendable {
    let name: String
    let branch: String?
    let path: String?
}

actor LFDEventService {
    private var connection: NWConnection?
    private let socketPath = FileManager.default.homeDirectoryForCurrentUser
        .appendingPathComponent(".lf/lfd.sock")
    private var onEvent: (@Sendable (WorktreeEvent) -> Void)?

    func subscribe(
        to patterns: [String],
        onEvent: @escaping @Sendable (WorktreeEvent) -> Void
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
                            let event = WorktreeEvent(
                                name: eventName,
                                branch: eventData["branch"] as? String,
                                path: eventData["path"] as? String
                            )
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

    private func handleEvent(_ event: WorktreeEvent) {
        onEvent?(event)
    }

    func disconnect() {
        connection?.cancel()
        connection = nil
        onEvent = nil
    }
}
