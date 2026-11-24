#if os(macOS)
import Darwin
import Foundation
import Loopflow
import Synchronization

/// Shared Work operations use the server over one connection per location;
/// service errors remain visible.
enum ServerClient {
    /// How long a read may go unanswered. A read has no effect to lose, so it
    /// can give up; writes wait, because their outcome would become unknown.
    static let readDeadline = Duration.seconds(60)

    /// Read from the link selected for this location.
    static func read(_ query: ServedRead, location: ServerLocation = .shared,
                     deadline: Duration = readDeadline) async throws -> ReadOutput {
        try Task.checkCancellation()
        let link = try await links.link(for: location)
        return try await link.read(query, deadline: deadline)
    }

    /// What the write returned, as JSON: what `lf <write> --json` prints.
    static func write(_ change: ServedWrite, location: ServerLocation = .shared) async throws -> String {
        String(decoding: try await request(.command(change), location: location).bytes, as: UTF8.self)
    }

    /// A change under its request ID; a repeat after a lost answer runs nothing.
    static func change(_ change: ServedWrite, requestID: String, location: ServerLocation = .shared) async throws -> Revisions? {
        try Task.checkCancellation()
        let link = try await links.link(for: location)
        return try await link.write(change, requestID: requestID).revisions
    }

    static func submittedCreation(_ payload: Data) async throws -> String {
        let value = try JSONDecoder().decode(JSONValue.self, from: payload)
        return String(decoding: try await request(.submittedCreation(value), location: .shared).bytes, as: UTF8.self)
    }

    static func configuration(_ operation: ConfigOperation, location: ServerLocation = .shared) async throws -> RawJSON {
        try await request(.config(operation), location: location)
    }

    static func edit(_ change: ConfigurationChange, destination: JSONValue, key: String,
                     expected: [ConfigurationEntry], location: ServerLocation) async throws -> ConfigurationEditReceipt {
        try await configuration(.configEdit(destination: destination, key: key, expected: expected, change: change),
                                location: location).decode()
    }

    private static func request(_ operation: ServerOperation, location: ServerLocation) async throws -> RawJSON {
        try Task.checkCancellation()
        let link = try await links.link(for: location)
        return try await link.request(operation)
    }

    /// A subscription on the location's connection; `checkpoint` confirms a reading,
    /// `cancel` ends it. After a reading, connection loss resumes the same stream.
    static func subscribe(_ query: ServedRead, location: ServerLocation = .shared) -> LineSubscription {
        // The newest link's sentence for a reading this build can't decode; nil before one connects.
        let unreadable = Mutex<String?>(nil)
        let watch = ServerLink.subscribe(query, connect: {
            let link = try await links.link(for: location)
            unreadable.withLock { $0 = link.unreadable }
            return link
        }) { update -> Data? in
            guard case .reading(let frame) = update else { return nil }
            return Data(frame.bytes)
        }
        return LineSubscription(lines: watch.updates, checkpoint: { id in
            guard let number = UInt64(exactly: id) else { watch.cancel(); return }
            watch.checkpoint(number)
        }, cancel: watch.cancel, unreadable: { unreadable.withLock { $0 } })
    }

    /// Readings and occurrences share one stream so the consumer applies its
    /// baseline before events. Reconnect replaces state without replaying events.
    static func subscribeEvents(_ query: ServedRead, location: ServerLocation = .shared) -> ServerWatch<ServerSubscriptionUpdate> {
        ServerLink.subscribe(query, events: true, connect: { try await links.link(for: location) }) { $0 }
    }

    private static let links = ServerLinks()
}

/// One live connection per server location, reopened on demand with bounded
/// backoff after consecutive failures to open.
private actor ServerLinks {
    private struct Owner { weak var location: ServerLocation? }
    private var owners: [ObjectIdentifier: Owner] = [:]
    private var links: [ObjectIdentifier: Task<ServerLink, any Error>] = [:]
    private var failures: [ObjectIdentifier: Int] = [:]

    func link(for location: ServerLocation) async throws -> ServerLink {
        let key = ObjectIdentifier(location)
        // A freed location's identifier can name a new one at the same
        // address; that one starts without the old one's connection.
        if owners[key]?.location !== location {
            links[key] = nil
            failures[key] = nil
            owners[key] = Owner(location: location)
        }
        while let opening = links[key] {
            // A failed open answers everyone it kept waiting; only a link that
            // opened and later closed is worth another attempt.
            let link = try await opening.value
            if await !link.isClosed { return link }
            if links[key] == opening { links[key] = nil }
        }
        let attempt = failures[key] ?? 0
        let opening = Task {
            if attempt > 0 { try await Task.sleep(for: .milliseconds(min(100 << (attempt - 1), 3200))) }
            let path = try await location.path()
            return try await ServerLink.open(.unix(path: path))
        }
        links[key] = opening
        do {
            let link = try await opening.value
            failures[key] = 0
            return link
        } catch {
            failures[key] = attempt + 1
            if links[key] == opening { links[key] = nil }
            throw error
        }
    }
}

actor ServerLocation {
    static let shared = ServerLocation(startsServer: LocalWaveAgentLauncher.machineLf().startsServer)
    private let query: @Sendable ([String]) async throws -> String
    private var endpoint: Task<String, any Error>?
    private let startsServer: Bool
    private var setupError: String?

    init(
        startsServer: Bool,
        query: @escaping @Sendable ([String]) async throws -> String = {
            try await LocalWaveAgentLauncher.queryLf($0, cwd: "/")
        }
    ) {
        self.startsServer = startsServer
        self.query = query
    }

    func path() async throws -> String {
        if let endpoint { return try await endpoint.value }
        // Keep a successful endpoint. After a failed install, keep its diagnosis
        // and check status on subsequent attempts.
        let prepare = startsServer && setupError == nil
        let previousSetupError = setupError
        let task = Task {
            do {
                let result = try await query(prepare ? ["install", "server"] : ["server", "status"])
                struct Status: Decodable {
                    let socket: String
                    let ready: Bool
                    let error: String?
                }
                let status = try JSONDecoder().decode(Status.self, from: Data(result.utf8))
                guard status.ready else {
                    if !startsServer {
                        let guidance = "Loopflow's server isn't running. Open Loopflow, or run `lf install server`."
                        throw RegistryQueryError([guidance, status.error].compactMap { $0 }.joined(separator: " "))
                    }
                    throw RegistryQueryError(status.error ?? "Loopflow server unavailable")
                }
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

extension ServerEndpoint {
    /// The Machine's server on its private Unix socket.
    static func unix(path: String) -> ServerEndpoint {
        ServerEndpoint(
            client: ServerClientIdentity(kind: "desktop", platform: "macos",
                                          name: Bundle.main.object(forInfoDictionaryKey: "CFBundleName") as? String ?? "Loopflow",
                                          version: "\(LaunchJournal.appVersion)+\(DesktopLog.sourceRevision)"),
            control: { try UnixSocketCarrier(path: path, limit: ServerProtocol.maxMessage) },
            ticketed: { try UnixSocketCarrier(path: path, limit: ServerProtocol.maxBulk) })
    }
}

/// Newline-delimited messages over a same-account Unix socket. One queue reads,
/// one writes; blocking calls never run on the caller's executor.
private final class UnixSocketCarrier: ServerCarrier, @unchecked Sendable {
    private let fd: Int32
    private let reads = DispatchQueue(label: "loopflow.server.read")
    private let writes = DispatchQueue(label: "loopflow.server.write")
    /// Touched only on `reads`.
    private var reader: LineReader

    init(path: String, limit: Int) throws {
        fd = try Self.connect(path)
        reader = LineReader(fd: fd, limit: limit)
    }

    deinit { Darwin.close(fd) }

    func send(_ message: Data) async throws {
        var line = message
        line.append(10)
        try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Void, any Error>) in
            writes.async { [self, line] in
                do {
                    try Self.write(fd, line)
                    continuation.resume()
                } catch {
                    Darwin.shutdown(fd, SHUT_RDWR)
                    continuation.resume(throwing: error)
                }
            }
        }
    }

    func receive() async throws -> Data? {
        try await withCheckedThrowingContinuation { continuation in
            reads.async { [self] in continuation.resume(with: Result { try reader.next() }) }
        }
    }

    /// Wakes a blocked read; the descriptor closes once nothing uses it.
    func close() async { Darwin.shutdown(fd, SHUT_RDWR) }

    private static func write(_ fd: Int32, _ bytes: Data) throws {
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

    /// A same-account peer; the socket's directory is private.
    private static func connect(_ path: String) throws -> Int32 {
        var address = sockaddr_un()
        address.sun_family = sa_family_t(AF_UNIX)
        guard path.utf8.count < MemoryLayout.size(ofValue: address.sun_path) else {
            throw RegistryQueryError("Server socket path is too long")
        }
        withUnsafeMutableBytes(of: &address.sun_path) { $0.copyBytes(from: path.utf8) }
        let fd = Darwin.socket(AF_UNIX, SOCK_STREAM, 0)
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
        return fd
    }
}

/// Newline-delimited messages read from one descriptor by one queue.
private struct LineReader {
    let fd: Int32
    let limit: Int
    private var buffer = Data()
    /// Bytes already searched for a newline.
    private var scanned = 0
    private var ended = false

    init(fd: Int32, limit: Int) {
        self.fd = fd
        self.limit = limit
    }

    /// The next message, or nil once the peer closed after a whole message.
    mutating func next() throws -> Data? {
        while true {
            if let newline = buffer[(buffer.startIndex + scanned)...].firstIndex(of: 10) {
                guard newline - buffer.startIndex <= limit else {
                    throw RegistryQueryError("Server message exceeds \(limit >> 20) MiB")
                }
                let line = buffer.subdata(in: buffer.startIndex..<newline)
                buffer.removeSubrange(buffer.startIndex...newline)
                scanned = 0
                return line
            }
            scanned = buffer.count
            guard buffer.count <= limit else { throw RegistryQueryError("Server message exceeds \(limit >> 20) MiB") }
            if ended {
                guard buffer.isEmpty else { throw RegistryQueryError("Server disconnected mid-message; refresh state before retrying a command") }
                return nil
            }
            var chunk = [UInt8](repeating: 0, count: 65536)
            let received = chunk.withUnsafeMutableBytes { Darwin.read(fd, $0.baseAddress!, $0.count) }
            if received < 0 {
                if errno == EINTR { continue }
                throw RegistryQueryError(ServerLink.disconnected)
            }
            if received == 0 { ended = true; continue }
            buffer.append(contentsOf: chunk[..<received])
        }
    }
}
#endif
