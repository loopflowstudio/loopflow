import Darwin
import Foundation

/// A surface's live presentation; it conveys no Session authority.
struct ServerConnectionReading: Decodable, Equatable {
    enum State: String, Decodable { case connecting, connected, moving, reconnecting, unavailable, ended }
    let generation: UInt64
    let state: State
    let input: Bool
    let nativeReady: Bool

    enum CodingKeys: String, CodingKey { case generation, state, input, message; case nativeReady = "native_ready" }
    let message: String?

    static let openingTitle = "Opening… Input paused."
    var acceptsInput: Bool { state == .connected && input && nativeReady }
    var canMoveHere: Bool { state == .connected && !input }
    var title: String? {
        switch state {
        case .connecting: Self.openingTitle
        case .connected: input ? (nativeReady ? nil : Self.openingTitle) : "Read-only. Move here to type."
        case .moving: "Moving input here… Input paused."
        case .reconnecting: "Reconnecting… Input paused; last screen may be stale."
        case .unavailable: message ?? "Session unavailable. Input paused; last screen may be stale."
        case .ended: "Session ended."
        }
    }


}

@MainActor
final class ServerConnection {
    private let channel = Channel()
    var directory: URL { channel.directory }
    var path: String { directory.appendingPathComponent("status.sock").path }
    private(set) var reading = ServerConnectionReading(generation: 0, state: .connecting, input: false, nativeReady: false, message: nil)
    private var changed: (@MainActor (ServerConnectionReading) -> Void)?
    private var pendingMove: UInt64?
    private var received = Data()
    var canRetry: Bool { channel.fd >= 0 && reading.state == .unavailable }

    func start(changed: @escaping @MainActor (ServerConnectionReading) -> Void) throws {
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: false,
                                               attributes: [.posixPermissions: 0o700])
        var address = sockaddr_un()
        address.sun_family = sa_family_t(AF_UNIX)
        guard path.utf8.count < MemoryLayout.size(ofValue: address.sun_path) else {
            channel.stop()
            throw POSIXError(.ENAMETOOLONG)
        }
        withUnsafeMutableBytes(of: &address.sun_path) { $0.copyBytes(from: path.utf8) }
        let fd = socket(AF_UNIX, SOCK_STREAM, 0)
        guard fd >= 0 else {
            let error = POSIXError(POSIXErrorCode(rawValue: errno) ?? .EIO)
            channel.stop()
            throw error
        }
        let bound = withUnsafePointer(to: &address) {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                bind(fd, $0, socklen_t(MemoryLayout<sockaddr_un>.size))
            }
        }
        guard bound == 0, chmod(path, 0o600) == 0, listen(fd, 1) == 0,
              fcntl(fd, F_SETFL, O_NONBLOCK) == 0 else {
            let error = POSIXError(POSIXErrorCode(rawValue: errno) ?? .EIO)
            close(fd)
            channel.stop()
            throw error
        }
        _ = fcntl(fd, F_SETFD, FD_CLOEXEC)
        let source = DispatchSource.makeReadSource(fileDescriptor: fd, queue: .main)
        channel.listener = source
        source.setEventHandler { [weak self] in
            MainActor.assumeIsolated { self?.accept(fd) }
        }
        source.setCancelHandler { close(fd) }
        self.changed = changed
        source.resume()
        changed(reading)
    }

    private func accept(_ listener: Int32) {
        let fd = Darwin.accept(listener, nil, nil)
        guard fd >= 0 else { return }
        var uid: uid_t = 0
        var gid: gid_t = 0
        guard getpeereid(fd, &uid, &gid) == 0, uid == getuid(),
              fcntl(fd, F_SETFL, O_NONBLOCK) == 0 else { close(fd); return }
        _ = fcntl(fd, F_SETFD, FD_CLOEXEC)
        var enabled: Int32 = 1
        setsockopt(fd, SOL_SOCKET, SO_NOSIGPIPE, &enabled, socklen_t(MemoryLayout<Int32>.size))
        if channel.fd >= 0 { statusUnavailable() }
        received.removeAll()
        pendingMove = nil
        channel.fd = fd
        let source = DispatchSource.makeReadSource(fileDescriptor: fd, queue: .main)
        channel.reader = source
        source.setEventHandler { [weak self] in
            MainActor.assumeIsolated { self?.readStatus(fd) }
        }
        source.setCancelHandler { close(fd) }
        source.resume()
    }

    private func readStatus(_ fd: Int32) {
        guard channel.fd == fd else { return }
        var bytes = [UInt8](repeating: 0, count: 4096)
        while true {
            let count = Darwin.read(fd, &bytes, bytes.count)
            if count < 0 && (errno == EAGAIN || errno == EWOULDBLOCK) { return }
            guard count > 0 else { statusUnavailable(); return }
            received.append(contentsOf: bytes.prefix(count))
            guard received.count <= 65536 else { statusUnavailable(); return }
            while let end = received.firstIndex(of: 10) {
                let line = Data(received[..<end])
                received.removeSubrange(...end)
                guard let next = try? JSONDecoder().decode(ServerConnectionReading.self, from: line) else {
                    statusUnavailable(); return
                }
                guard next.generation >= reading.generation,
                      pendingMove.map({ next.generation > $0 }) ?? true else { continue }
                pendingMove = nil
                reading = next
                changed?(reading)
            }
        }
    }

    private func statusUnavailable() {
        channel.disconnect()
        received.removeAll()
        pendingMove = nil
        guard reading.state != .ended else { return }
        reading = ServerConnectionReading(generation: reading.generation, state: .unavailable, input: false, nativeReady: false,
                                          message: "Connection status unavailable. Input paused.")
        changed?(reading)
    }

    private func send(_ action: [String: Any]) -> Bool {
        guard channel.fd >= 0, var bytes = try? JSONSerialization.data(withJSONObject: action) else { return false }
        bytes.append(10)
        let sent = bytes.withUnsafeBytes { Darwin.send(channel.fd, $0.baseAddress, $0.count, 0) }
        guard sent == bytes.count else { statusUnavailable(); return false }
        return true
    }

    func retry() {
        guard canRetry else { return }
        _ = send(["kind": "retry"])
    }

    func moveHere() {
        guard reading.canMoveHere, send(["kind": "move", "generation": reading.generation]) else { return }
        pendingMove = reading.generation
        reading = ServerConnectionReading(generation: reading.generation, state: .moving, input: false, nativeReady: false, message: nil)
        changed?(reading)
    }

    func stop() {
        channel.stop()
        received.removeAll()
        pendingMove = nil
        changed = nil
    }

    // Resource cleanup may run on the thread releasing the last surface.
    private final class Channel {
        let directory = URL(fileURLWithPath: "/tmp").appendingPathComponent("lf-view-" + UUID().uuidString)
        var listener: DispatchSourceRead?
        var reader: DispatchSourceRead?
        var fd: Int32 = -1

        func disconnect() {
            if fd >= 0 { shutdown(fd, SHUT_RDWR) }
            reader?.cancel()
            reader = nil
            fd = -1
        }

        func stop() {
            disconnect()
            listener?.cancel()
            listener = nil
            // The relay observes closure separately from a broken status stream.
            let closed = directory.appendingPathExtension("closed")
            do {
                try FileManager.default.moveItem(at: directory, to: closed)
                try FileManager.default.removeItem(at: closed)
            } catch {}
        }

        deinit { stop() }
    }
}
