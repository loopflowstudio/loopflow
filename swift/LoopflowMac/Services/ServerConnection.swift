import Foundation

/// A surface owns this temporary mailbox; it conveys no Session authority.
struct ServerConnectionReading: Decodable, Equatable {
    enum State: String, Decodable { case connected, moving, reconnecting, unavailable, ended }
    let generation: UInt64
    let state: State
    let input: Bool
    let message: String?

    var acceptsInput: Bool { state == .connected && input }
    var canMoveHere: Bool { state == .connected && !input }
    var title: String? {
        switch state {
        case .connected: input ? nil : "Read-only. Move here to type."
        case .moving: "Moving input here… Input paused."
        case .reconnecting: "Reconnecting… Input paused; last screen may be stale."
        case .unavailable: message ?? "Session unavailable. Last screen is stale."
        case .ended: "Session ended."
        }
    }
}

@MainActor
final class ServerConnection {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent("lf-view-" + UUID().uuidString)
    private var statusURL: URL { directory.appendingPathComponent("status.json") }
    var path: String { statusURL.path }
    private(set) var reading = ServerConnectionReading(generation: 0, state: .reconnecting, input: false, message: nil)
    private var observation: Task<Void, Never>?
    private var changed: (@MainActor (ServerConnectionReading) -> Void)?
    private var pendingMove: UInt64?

    func start(changed: @escaping @MainActor (ServerConnectionReading) -> Void) throws {
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: false,
                                               attributes: [.posixPermissions: 0o700])
        self.changed = changed
        changed(reading)
        observation = Task { [weak self] in
            while !Task.isCancelled {
                guard let self else { return }
                if let data = FileManager.default.contents(atPath: path),
                   let next = try? JSONDecoder().decode(ServerConnectionReading.self, from: data),
                   pendingMove.map({ next.generation > $0 }) ?? true,
                   next != reading {
                    pendingMove = nil
                    reading = next
                    changed(next)
                }
                try? await Task.sleep(for: .milliseconds(100))
            }
        }
    }

    func retry() {
        guard observation != nil, reading.state == .unavailable else { return }
        try? Data().write(to: statusURL.deletingPathExtension().appendingPathExtension("retry"), options: .atomic)
    }

    func moveHere() {
        guard observation != nil, reading.canMoveHere else { return }
        do {
            let action = statusURL.deletingPathExtension().appendingPathExtension("move")
            try JSONEncoder().encode(reading.generation).write(to: action, options: .atomic)
            pendingMove = reading.generation
            reading = ServerConnectionReading(generation: reading.generation, state: .moving, input: false, message: nil)
            changed?(reading)
        } catch {
            // A removed mailbox means this surface has closed. No action is queued.
        }
    }

    func stop() {
        observation?.cancel()
        observation = nil
        pendingMove = nil
        changed = nil
        try? FileManager.default.removeItem(at: directory)
    }
}
