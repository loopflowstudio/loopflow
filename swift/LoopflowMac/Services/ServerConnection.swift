import Darwin
import Foundation

/// A surface owns this temporary mailbox; it conveys no Session authority.
struct ServerConnectionReading: Decodable, Equatable {
    enum State: String, Decodable { case connecting, connected, moving, reconnecting, unavailable, ended }
    let generation: UInt64
    let state: State
    let input: Bool
    let message: String?

    var acceptsInput: Bool { state == .connected && input }
    var canMoveHere: Bool { state == .connected && !input }
    var title: String? {
        switch state {
        case .connecting: "Opening session…"
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
    private let mailbox = Mailbox()
    var directory: URL { mailbox.directory }
    var path: String { directory.appendingPathComponent("status.json").path }
    private(set) var reading = ServerConnectionReading(generation: 0, state: .connecting, input: false, message: nil)
    private var changed: (@MainActor (ServerConnectionReading) -> Void)?
    private var pendingMove: UInt64?

    func start(changed: @escaping @MainActor (ServerConnectionReading) -> Void) throws {
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: false,
                                               attributes: [.posixPermissions: 0o700])
        let fd = open(directory.path, O_EVTONLY | O_CLOEXEC)
        guard fd >= 0 else {
            let error = POSIXError(POSIXErrorCode(rawValue: errno) ?? .EIO)
            mailbox.stop()
            throw error
        }
        // The relay replaces status.json atomically, so watch its parent inode.
        let source = DispatchSource.makeFileSystemObjectSource(
            fileDescriptor: fd, eventMask: .write, queue: .main)
        mailbox.source = source
        source.setEventHandler { [weak self] in
            MainActor.assumeIsolated { self?.readStatus() }
        }
        source.setCancelHandler { close(fd) }
        self.changed = changed
        source.resume()
        changed(reading)
        readStatus()
    }

    private func readStatus() {
        guard mailbox.source != nil,
              let data = FileManager.default.contents(atPath: path),
              let next = try? JSONDecoder().decode(ServerConnectionReading.self, from: data),
              pendingMove.map({ next.generation > $0 }) ?? true,
              next != reading else { return }
        pendingMove = nil
        reading = next
        changed?(next)
    }

    func retry() {
        guard mailbox.source != nil, reading.state == .unavailable else { return }
        try? Data().write(to: directory.appendingPathComponent("status.retry"), options: .atomic)
    }

    func moveHere() {
        guard mailbox.source != nil, reading.canMoveHere else { return }
        do {
            let action = directory.appendingPathComponent("status.move")
            try JSONEncoder().encode(reading.generation).write(to: action, options: .atomic)
            pendingMove = reading.generation
            reading = ServerConnectionReading(generation: reading.generation, state: .moving, input: false, message: nil)
            changed?(reading)
        } catch {
            // A removed mailbox means this surface has closed. No action is queued.
        }
    }

    func stop() {
        mailbox.stop()
        pendingMove = nil
        changed = nil
    }

    // Mutated on the main actor; cancellation and cleanup can also run on the
    // thread that releases the last owner, without an actor-isolated deinit.
    private final class Mailbox {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent("lf-view-" + UUID().uuidString)
        var source: DispatchSourceFileSystemObject?

        func stop() {
            source?.cancel()
            source = nil
            // Remove the address before its contents: the relay can still be
            // publishing as the pane closes.
            let closed = directory.appendingPathExtension("closed")
            do {
                try FileManager.default.moveItem(at: directory, to: closed)
                try FileManager.default.removeItem(at: closed)
            } catch {
                // Repeated stop has no mailbox left to remove.
            }
        }

        deinit { stop() }
    }
}
