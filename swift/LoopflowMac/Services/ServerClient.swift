#if os(macOS)
import Darwin
import Foundation
import Loopflow

/// Shared Work operations use the server; service errors remain visible.
enum ServerClient {
    static func read(_ query: ServedRead, location: ServerLocation = .shared) async throws -> String {
        let response = try await request(.read(query), location: location)
        guard case .read(let output) = response else { throw RegistryQueryError("Unexpected Server reading") }
        return output
    }

    static func write(_ change: ServedWrite, location: ServerLocation = .shared) async throws -> String {
        let response = try await request(.command(change), location: location)
        guard case .command(_, let output, let stderr, let code) = response else {
            throw RegistryQueryError("Server command outcome unknown; inspect the affected work before retrying")
        }
        guard code == 0 else { throw RegistryQueryError(stderr, stdout: output) }
        return output
    }

    private static func request(_ operation: ServerOperation, location: ServerLocation = .shared) async throws -> ServerReply {
        try Task.checkCancellation()
        let path = try await location.path()
        try Task.checkCancellation()
        let request = ServerRequestConnection()
        return try await withTaskCancellationHandler {
            try await withCheckedThrowingContinuation { continuation in
                Thread.detachNewThread {
                    do {
                        let connection = try ServerSocket(path: path)
                        defer { connection.close() }
                        try request.connect(connection)
                        let id = UUID().uuidString
                        try connection.send(JSONEncoder().encode(ServerRequest(version: 1, id: id, operation: operation)))
                        let reply = try connection.receive(id: id).result
                        try request.checkCancellation()
                        continuation.resume(returning: reply)
                    } catch {
                        do { try request.checkCancellation() }
                        catch { continuation.resume(throwing: error); return }
                        continuation.resume(throwing: error)
                    }
                }
            }
        } onCancel: {
            request.cancel()
        }
    }

    static func watch(_ query: ServedRead, location: ServerLocation = .shared) -> LineWatch {
        let watch = ServerWatch(query, location: location)
        return LineWatch(lines: watch.lines, send: watch.send, cancel: watch.cancel)
    }
}

/// Cancellation releases this viewer's wait, never the command's execution.
private final class ServerRequestConnection: @unchecked Sendable {
    private let lock = NSLock()
    private var socket: ServerSocket?
    private var cancelled = false

    func connect(_ socket: ServerSocket) throws {
        try lock.withLock {
            guard !cancelled else { throw CancellationError() }
            self.socket = socket
        }
    }

    func checkCancellation() throws {
        try lock.withLock {
            if cancelled { throw CancellationError() }
        }
    }

    func cancel() {
        lock.withLock {
            cancelled = true
            socket?.shutdown()
        }
    }
}

actor ServerLocation {
    static let shared = ServerLocation()
    private let query: @Sendable ([String]) async throws -> String
    private var endpoint: Task<String, any Error>?
    private var setupError: String?

    init(query: @escaping @Sendable ([String]) async throws -> String = {
        try await LocalWaveAgentLauncher.queryLf($0, cwd: "/")
    }) {
        self.query = query
    }

    func path() async throws -> String {
        if let endpoint { return try await endpoint.value }
        // The shared task retains a successful endpoint. A failed initial lookup
        // retains its diagnosis instead; later lookups only observe status.
        let prepare = setupError == nil
        let previousSetupError = setupError
        let task = Task {
            do {
                let result = try await query(prepare ? ["install", "server"] : ["server", "status"])
                struct Status: Decodable {
                    let socket: String
                    let ready: Bool
                    let instance: String?
                    let error: String?
                }
                let status = try JSONDecoder().decode(Status.self, from: Data(result.utf8))
                guard status.ready else { throw RegistryQueryError(status.error ?? "Loopflow server unavailable") }
                return status.socket
            } catch {
                // Every concurrent waiter gets the same actionable diagnosis.
                if let previousSetupError {
                    throw RegistryQueryError("Initial setup failed: \(previousSetupError). Current server: \(error.localizedDescription)")
                }
                throw error
            }
        }
        endpoint = task
        do {
            return try await task.value
        } catch {
            endpoint = nil
            if prepare {
                setupError = error.localizedDescription
            }
            throw error
        }
    }
}

/// One bounded stream and a single reader. Shutdown interrupts the read; only
/// the reader closes its descriptor, so cancellation cannot race descriptor reuse.
private final class ServerSocket: @unchecked Sendable {
    private let fd: Int32
    private let writes = NSLock()
    private let state = NSLock()
    private var closed = false
    static let maxFrame = 32 * 1024 * 1024

    init(path: String) throws {
        var address = sockaddr_un()
        address.sun_family = sa_family_t(AF_UNIX)
        guard path.utf8.count < MemoryLayout.size(ofValue: address.sun_path) else {
            throw RegistryQueryError("Server socket path is too long")
        }
        withUnsafeMutableBytes(of: &address.sun_path) { $0.copyBytes(from: path.utf8) }
        fd = Darwin.socket(AF_UNIX, SOCK_STREAM, 0)
        guard fd >= 0 else { throw RegistryQueryError("Cannot open Server socket") }
        let connected = withUnsafePointer(to: &address) { pointer in
            pointer.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                Darwin.connect(fd, $0, socklen_t(MemoryLayout<sockaddr_un>.size))
            }
        }
        var uid: uid_t = 0
        var gid: gid_t = 0
        guard connected == 0, getpeereid(fd, &uid, &gid) == 0, uid == geteuid() else {
            Darwin.close(fd)
            throw RegistryQueryError("Loopflow server unavailable or owned by another account")
        }
        var enabled: Int32 = 1
        setsockopt(fd, SOL_SOCKET, SO_NOSIGPIPE, &enabled, socklen_t(MemoryLayout<Int32>.size))
        var timeout = timeval(tv_sec: 5, tv_usec: 0)
        setsockopt(fd, SOL_SOCKET, SO_SNDTIMEO, &timeout, socklen_t(MemoryLayout<timeval>.size))
    }

    func send(_ body: Data) throws {
        try writes.withLock {
            guard !state.withLock({ closed }) else { throw RegistryQueryError("Server connection is closed") }
            guard body.count <= Self.maxFrame else { throw RegistryQueryError("Server request exceeds 32 MiB") }
            var length = UInt32(body.count).bigEndian
            var bytes = withUnsafeBytes(of: &length) { Data($0) }
            bytes.append(body)
            try bytes.withUnsafeBytes { buffer in
                var offset = 0
                while offset < buffer.count {
                    let sent = Darwin.write(fd, buffer.baseAddress!.advanced(by: offset), buffer.count - offset)
                    if sent < 0 && errno == EINTR { continue }
                    guard sent > 0 else { throw RegistryQueryError("Server send failed; delivery is unknown") }
                    offset += sent
                }
            }
        }
    }

    func receive(id: String) throws -> ServerResponse {
        let header = try read(4)
        let length = header.reduce(0) { ($0 << 8) | Int($1) }
        guard length <= Self.maxFrame else { throw RegistryQueryError("Server frame exceeds limit") }
        let response = try JSONDecoder().decode(ServerResponse.self, from: read(length))
        guard response.id == id else { throw RegistryQueryError("Server response identity mismatch") }
        if case .error(let code, let message) = response.result { throw RegistryQueryError("\(code): \(message)") }
        return response
    }

    private func read(_ count: Int) throws -> Data {
        var bytes = Data(count: count)
        try bytes.withUnsafeMutableBytes { buffer in
            var offset = 0
            while offset < count {
                let received = Darwin.read(fd, buffer.baseAddress!.advanced(by: offset), count - offset)
                if received < 0 && errno == EINTR { continue }
                guard received > 0 else { throw RegistryQueryError("Server disconnected; refresh state before retrying a command") }
                offset += received
            }
        }
        return bytes
    }

    func shutdown() { state.withLock { if !closed { _ = Darwin.shutdown(fd, SHUT_RDWR) } } }
    func close() {
        writes.withLock {
            state.withLock {
                if !closed { closed = true; Darwin.close(fd) }
            }
        }
    }
}

private final class ServerWatch: @unchecked Sendable {
    let lines: AsyncThrowingStream<Data, any Error>
    private let continuation: AsyncThrowingStream<Data, any Error>.Continuation
    private let lock = NSLock()
    private var connection: ServerSocket?
    private var pending: [Data] = []
    private var stopped = false
    private var queuedWrites = 0
    private let writer = DispatchQueue(label: "studio.loopflow.server-checkpoints")

    init(_ query: ServedRead, location: ServerLocation) {
        // Never drop a checkpoint silently. A slow consumer reconnects with a
        // fresh reading, using the existing WorkObservation recovery path.
        (lines, continuation) = AsyncThrowingStream.makeStream(bufferingPolicy: .bufferingOldest(8))
        continuation.onTermination = { [weak self] _ in self?.stop() }
        Task {
            do {
                let path = try await location.path()
                Thread.detachNewThread { self.run(path: path, query: query) }
            } catch { continuation.finish(throwing: error) }
        }
    }

    private func run(path: String, query: ServedRead) {
        do {
            let socket = try ServerSocket(path: path)
            defer { socket.close() }
            let id = UUID().uuidString
            try socket.send(JSONEncoder().encode(ServerRequest(version: 1, id: id, operation: .subscribe(query, generation: 1))))
            let start = try lock.withLock {
                guard !stopped else { return false }
                connection = socket
                for frame in pending { try socket.send(frame) }
                pending = []
                return true
            }
            guard start else { return }
            while true {
                let response = try socket.receive(id: id)
                guard case .reading(let generation, let frame) = response.result, generation == 1 else {
                    throw RegistryQueryError("Unexpected Server subscription generation")
                }
                if case .dropped = continuation.yield(try JSONEncoder().encode(frame)) {
                    throw RegistryQueryError("Server subscription fell behind; reconnect for current Work")
                }
            }
        } catch { continuation.finish(throwing: error) }
        stop()
    }

    func send(_ data: Data) {
        let admitted = lock.withLock {
            guard !stopped, queuedWrites + pending.count < 64 else { return false }
            queuedWrites += 1
            return true
        }
        guard admitted else {
            continuation.finish(throwing: RegistryQueryError("Server checkpoints exceeded limit or connection closed"))
            stop()
            return
        }
        writer.async {
            let socket: ServerSocket? = self.lock.withLock {
                self.queuedWrites -= 1
                guard !self.stopped else { return nil }
                if let socket = self.connection { return socket }
                self.pending.append(data)
                return nil
            }
            do { try socket?.send(data) }
            catch { self.continuation.finish(throwing: error); self.stop() }
        }
    }
    func cancel() async { stop() }
    private func stop() {
        lock.withLock {
            stopped = true
            connection?.shutdown()
            connection = nil
            pending = []
        }
        continuation.finish()
    }
}
#endif
