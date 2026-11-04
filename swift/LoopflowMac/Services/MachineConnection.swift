import Foundation

/// A surface owns this temporary mailbox; it conveys no Session authority.
struct MachineConnectionReading: Decodable, Equatable {
    enum State: String, Decodable { case connected, reconnecting, unavailable, ended }
    let generation: UInt64
    let state: State
    let input: Bool
    let message: String?

    var acceptsInput: Bool { state == .connected && input }
    var title: String? {
        switch state {
        case .connected: input ? nil : "Read-only — input is in another viewer. Use Move here to take control."
        case .reconnecting: "Reconnecting… Input paused; last screen may be stale."
        case .unavailable: message ?? "Session unavailable. Last screen is stale."
        case .ended: "Session ended."
        }
    }
}

@MainActor
final class MachineConnection {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent("lf-view-" + UUID().uuidString)
    var path: String { directory.appendingPathComponent("status.json").path }
    private(set) var reading = MachineConnectionReading(generation: 0, state: .reconnecting, input: false, message: nil)
    private var observation: Task<Void, Never>?

    func start(changed: @escaping @MainActor (MachineConnectionReading) -> Void) throws {
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: false,
                                               attributes: [.posixPermissions: 0o700])
        changed(reading)
        observation = Task { [weak self] in
            while !Task.isCancelled {
                guard let self else { return }
                if let data = FileManager.default.contents(atPath: path),
                   let next = try? JSONDecoder().decode(MachineConnectionReading.self, from: data),
                   next != reading {
                    reading = next
                    changed(next)
                }
                try? await Task.sleep(for: .milliseconds(100))
            }
        }
    }

    func retry() {
        try? Data().write(to: URL(fileURLWithPath: path).deletingPathExtension().appendingPathExtension("retry"), options: .atomic)
    }

    func stop() {
        observation?.cancel()
        observation = nil
        try? FileManager.default.removeItem(at: directory)
    }
}
