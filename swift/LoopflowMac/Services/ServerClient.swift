#if os(macOS)
import Darwin
import Foundation
import Loopflow

/// Shared Work operations use the server over one connection per location;
/// service errors remain visible.
enum ServerClient {
    /// How long a read may go unanswered. A read has no effect to lose, so it
    /// can give up; writes wait, because their outcome would become unknown.
    static let readDeadline = Duration.seconds(60)

    /// What `lf <read>` prints: bare text for a string value, JSON otherwise.
    static func read(_ query: ServedRead, location: ServerLocation = .shared,
                     deadline: Duration = readDeadline) async throws -> String {
        try Task.checkCancellation()
        let link = try await links.link(for: location)
        let value = try await link.read(.read(query), deadline: deadline)
        if case .string(let text) = value { return text }
        return String(decoding: try JSONEncoder().encode(value), as: UTF8.self)
    }

    /// What the write returned, as JSON: what `lf <write> --json` prints.
    static func write(_ change: ServedWrite, location: ServerLocation = .shared) async throws -> String {
        String(decoding: try JSONEncoder().encode(await request(.command(change), location: location)), as: UTF8.self)
    }

    static func configuration(_ operation: ConfigOperation, location: ServerLocation = .shared) async throws -> Data {
        try JSONEncoder().encode(await request(.config(operation), location: location))
    }

    private static func request(_ operation: ServerOperation, location: ServerLocation) async throws -> JSONValue {
        try Task.checkCancellation()
        let link = try await links.link(for: location)
        return try await link.request(operation)
    }

    /// A subscription on the location's connection; `send` carries checkpoints,
    /// `cancel` ends it. After a reading, connection loss resumes the same stream.
    static func subscribe(_ query: ServedRead, location: ServerLocation = .shared) -> LineSubscription {
        // Overflow ends the stream; its next consumer starts with a fresh reading.
        let (lines, continuation) = AsyncThrowingStream<Data, any Error>.makeStream(bufferingPolicy: .bufferingOldest(8))
        let subscription = ServerSubscription(continuation, query: query) {
            try await links.link(for: location)
        }
        Task {
            do { try await subscription.start() }
            catch { subscription.finish(throwing: error) }
        }
        return LineSubscription(lines: lines, send: subscription.send, cancel: subscription.cancel)
    }

    private static let links = ServerLinks()
}

/// One live connection per server location, reopened on demand with bounded
/// backoff after consecutive failures to open.
private actor ServerLinks {
    private var links: [ObjectIdentifier: Task<ServerLink, any Error>] = [:]
    private var failures: [ObjectIdentifier: Int] = [:]

    func link(for location: ServerLocation) async throws -> ServerLink {
        let key = ObjectIdentifier(location)
        while let opening = links[key] {
            // A failed open answers everyone it kept waiting; only a link that
            // opened and later closed is worth another attempt.
            let link = try await opening.value
            if !link.isClosed { return link }
            if links[key] == opening { links[key] = nil }
        }
        let attempt = failures[key] ?? 0
        let opening = Task {
            if attempt > 0 { try await Task.sleep(for: .milliseconds(min(100 << (attempt - 1), 3200))) }
            let path = try await location.path()
            return try await ServerLink.open(path: path)
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

/// One server connection: hello and welcome once, then requests, subscriptions
/// and cancels by id. One reader thread demultiplexes answers; writes serialize.
/// Losing the connection fails pending requests; watches resume their retained stream.
/// Messages over the control limit travel on ticketed connections of their own.
final class ServerLink: @unchecked Sendable {
    let welcome: ServerWelcome
    private let path: String
    private let fd: Int32
    private let state = NSLock()
    private let writes = NSLock()
    private let delivery = NSRecursiveLock()
    private var order = ServerMessageOrder()
    private var pending: [String: CheckedContinuation<JSONValue, any Error>] = [:]
    private var subscriptions: [String: ServerSubscription] = [:]
    private var failure: (any Error)?
    private var unansweredReads = 0
    private var reader: LineReader

    /// Connect, say hello and read welcome off the caller's executor.
    static func open(path: String) async throws -> ServerLink {
        try await withCheckedThrowingContinuation { continuation in
            Thread.detachNewThread {
                do { continuation.resume(returning: try ServerLink(path: path)) } catch { continuation.resume(throwing: error) }
            }
        }
    }

    /// How long the server has to welcome the connection; the session's reads
    /// are then unbounded, so a stalled open releases `ServerLinks` to back off.
    static let handshakeSeconds = 10
    /// A lost connection: requests in flight have unknown outcomes, not failures.
    static let disconnected = "Server disconnected; refresh state before retrying a command"

    private init(path: String) throws {
        self.path = path
        fd = try Self.connect(path)
        var reader = LineReader(fd: fd)
        do {
            Self.setReceiveTimeout(fd, seconds: Self.handshakeSeconds)
            let hello = ServerHello(revision: ServerProtocol.revision, minRevision: ServerProtocol.minRevision, client: ServerClientIdentity(
                kind: "desktop", platform: "macos", name: "Loopflow", version: LaunchJournal.appVersion))
            try Self.write(fd, Self.line(.hello(hello)))
            guard let first = try reader.next() else {
                throw RegistryQueryError("Loopflow server closed the connection without answering; it may speak an older protocol; update the selected installation")
            }
            switch try JSONDecoder().decode(ServerMessage.self, from: first) {
            case .welcome(let welcome): self.welcome = welcome
            case .error(_, let code, let message, let data): throw ServerError(code: code, message: message, data: data)
            default: throw RegistryQueryError("Server did not welcome the connection")
            }
            Self.setReceiveTimeout(fd, seconds: 0)
        } catch {
            Darwin.close(fd)
            throw error
        }
        self.reader = reader
        Thread.detachNewThread { [self] in run() }
    }

    var isClosed: Bool { state.withLock { failure != nil } }

    /// Reads in a row that went unanswered before the link counts as stalled.
    static let stalledReads = 3

    /// `request` within `deadline`. The server may hang on one read and serve
    /// the rest; only `stalledReads` in a row end the connection, which fails
    /// its subscriptions too, so a stalled server shows as unavailable, not as current.
    func read(_ operation: ServerOperation, deadline: Duration) async throws -> JSONValue {
        struct Expired: Error {}
        do {
            let value = try await withThrowingTaskGroup(of: JSONValue.self) { group in
                group.addTask { try await self.request(operation) }
                group.addTask {
                    try await Task.sleep(for: deadline)
                    throw Expired()
                }
                defer { group.cancelAll() }
                return try await group.next()!
            }
            state.withLock { unansweredReads = 0 }
            return value
        } catch is Expired {
            let stalled = state.withLock {
                unansweredReads += 1
                return unansweredReads >= Self.stalledReads
            }
            if stalled { writes.withLock { _ = Darwin.shutdown(fd, SHUT_RDWR) } }
            throw RegistryQueryError("Loopflow server did not answer in time; check `lf server status`")
        }
    }

    /// One request and its answer. Cancelling the task sends `cancel` and
    /// releases the wait; a command already dispatched may still complete.
    func request(_ operation: ServerOperation) async throws -> JSONValue {
        let id = UUID().uuidString
        let line = try Self.line(.request(id: id, operation: operation, select: nil))
        if line.count <= ServerProtocol.maxMessage { return try await answer(id) { try write(line) } }
        // An upload ticket admits a second connection carrying this one request;
        // its answer arrives here under the request's own id.
        guard case .string(let ticket) = try await request(.requestUpload(bytes: line.count))["ticket"] else {
            throw RegistryQueryError("Server issued no upload ticket")
        }
        return try await answer(id) {
            Thread.detachNewThread { [self] in
                do { try Self.ticketed(path, ticket) { fd, _ in try Self.write(fd, line) } } catch { settle(id, throwing: error) }
            }
        }
    }

    private func answer(_ id: String, deliver: () throws -> Void) async throws -> JSONValue {
        try await withTaskCancellationHandler {
            try await withCheckedThrowingContinuation { continuation in
                let refused: (any Error)? = state.withLock {
                    if let failure { return failure }
                    if Task.isCancelled { return CancellationError() }
                    pending[id] = continuation
                    return nil
                }
                if let refused { continuation.resume(throwing: refused); return }
                do { try deliver() } catch { settle(id, throwing: error) }
            }
        } onCancel: {
            delivery.lock()
            defer { delivery.unlock() }
            order.discard(id)
            if let waiting = state.withLock({ pending.removeValue(forKey: id) }) {
                try? send(.cancel(id: id))
                waiting.resume(throwing: CancellationError())
            }
        }
    }

    /// Fail the request or subscription `id` without touching the connection.
    private func settle(_ id: String, throwing error: any Error) {
        delivery.lock()
        defer { delivery.unlock() }
        order.discard(id)
        if let waiting = state.withLock({ pending.removeValue(forKey: id) }) { waiting.resume(throwing: error); return }
        state.withLock { subscriptions.removeValue(forKey: id) }?.finish(throwing: error)
    }

    fileprivate func subscribe(_ subscription: ServerSubscription, query: ServedRead, resume: String?) throws {
        let refused: (any Error)? = state.withLock {
            if let failure { return failure }
            subscriptions[subscription.id] = subscription
            return nil
        }
        if let refused { throw refused }
        do { try send(.subscribe(id: subscription.id, operation: query, select: nil, resume: resume)) } catch {
            _ = state.withLock { subscriptions.removeValue(forKey: subscription.id) }
            throw error
        }
    }

    fileprivate func unsubscribe(_ id: String) {
        delivery.lock()
        defer { delivery.unlock() }
        order.discard(id)
        let alive = state.withLock {
            subscriptions.removeValue(forKey: id)
            return failure == nil
        }
        if alive { try? send(.cancel(id: id)) }
    }

    fileprivate func send(_ message: ServerClientMessage) throws {
        try write(try Self.line(message))
    }

    private func write(_ line: Data) throws {
        try writes.withLock {
            if let failure = state.withLock({ failure }) { throw failure }
            do { try Self.write(fd, line) } catch {
                // A torn line would corrupt whatever follows it: end the connection.
                Darwin.shutdown(fd, SHUT_RDWR)
                throw error
            }
        }
    }

    private func run() {
        let ended: any Error
        do {
            while let line = try reader.next() { try dispatch(line) }
            ended = RegistryQueryError(Self.disconnected)
        } catch { ended = error }
        fail(ended)
    }

    private func dispatch(_ line: Data) throws {
        try delivery.withLock {
            guard !isClosed, let message = try order.receive(line) else { return }
            try deliver(message)
        }
    }

    private func deliver(_ message: ServerMessage) throws {
        switch message {
        case .response(let id, _, let result):
            if let waiting = state.withLock({ pending.removeValue(forKey: id) }) { waiting.resume(returning: result) }
            // A null result confirms a registered subscription; anything else is
            // a late answer to a forgotten request.
        case .reading(let id, let checkpoint, let frame):
            state.withLock { subscriptions[id] }?.receive(frame, checkpoint: checkpoint)
        case .bulk(let id, let ticket, _):
            // Fetched off the reader thread, so other readings never wait behind it.
            Thread.detachNewThread { [self] in
                do {
                    let payload = try Self.ticketed(path, ticket) { _, reader in
                        guard let line = try reader.next() else { throw RegistryQueryError("Server closed the bulk connection without its message") }
                        return line
                    }
                    try delivery.withLock {
                        guard let (message, queued) = try order.complete(id, payload: payload) else { return }
                        try deliver(message)
                        for line in queued { try dispatch(line) }
                    }
                } catch { settle(id, throwing: error) }
            }
        case .ended(let id):
            state.withLock { subscriptions.removeValue(forKey: id) }?.end()
        case .error(let id?, let code, let message, let data):
            let refusal = ServerError(code: code, message: message, data: data)
            if let waiting = state.withLock({ pending.removeValue(forKey: id) }) { waiting.resume(throwing: refusal); return }
            state.withLock { subscriptions.removeValue(forKey: id) }?.finish(throwing: refusal)
        case .error(nil, let code, let message, let data):
            throw ServerError(code: code, message: message, data: data)
        case .welcome, .status, .bridged, .admitted:
            // A client connection is never a bridge; `lf session resume` opens those.
            throw RegistryQueryError("Server repeated its greeting; update the client")
        }
    }

    /// A ticketed connection: present `ticket`, read `admitted`, then `body` has
    /// the socket to itself. Every read waits at most the handshake bound.
    private static func ticketed<T>(_ path: String, _ ticket: String, _ body: (Int32, inout LineReader) throws -> T) throws -> T {
        let fd = try connect(path)
        defer { Darwin.close(fd) }
        setReceiveTimeout(fd, seconds: handshakeSeconds)
        try write(fd, try line(.attach(ticket: ticket)))
        var reader = LineReader(fd: fd, limit: ServerProtocol.maxBulk)
        guard let first = try reader.next() else { throw RegistryQueryError("Server closed the ticketed connection without admitting it") }
        switch try JSONDecoder().decode(ServerMessage.self, from: first) {
        case .admitted: break
        case .error(_, let code, let message, _): throw RegistryQueryError("\(code): \(message)")
        default: throw RegistryQueryError("Server did not admit the ticket; update the client")
        }
        // Admission and the first payload bytes may arrive in one read.
        return try body(fd, &reader)
    }

    private func fail(_ error: any Error) {
        delivery.lock()
        defer { delivery.unlock() }
        order = ServerMessageOrder()
        let (waiting, subscribing) = state.withLock {
            failure = error
            defer { pending = [:]; subscriptions = [:] }
            return (Array(pending.values), Array(subscriptions.values))
        }
        for continuation in waiting { continuation.resume(throwing: RegistryQueryError(Self.disconnected)) }
        for subscription in subscribing { subscription.disconnected(from: self, error: error) }
        _ = writes.withLock { Darwin.close(fd) }
    }

    private static func line(_ message: ServerClientMessage) throws -> Data {
        var line = try JSONEncoder().encode(message)
        guard line.count <= ServerProtocol.maxBulk else { throw RegistryQueryError("Server request exceeds 64 MiB") }
        line.append(10)
        return line
    }

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

    /// Zero seconds waits without limit.
    private static func setReceiveTimeout(_ fd: Int32, seconds: Int) {
        var timeout = timeval(tv_sec: seconds, tv_usec: 0)
        setsockopt(fd, SOL_SOCKET, SO_RCVTIMEO, &timeout, socklen_t(MemoryLayout<timeval>.size))
    }
}

/// Newline-delimited messages read from one descriptor by one thread.
private struct LineReader {
    let fd: Int32
    let limit: Int
    private var buffer = Data()
    /// Bytes already searched for a newline.
    private var scanned = 0
    private var ended = false

    init(fd: Int32, limit: Int = ServerProtocol.maxMessage) {
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
                if errno == EAGAIN || errno == EWOULDBLOCK {
                    throw RegistryQueryError("Loopflow server did not answer within \(ServerLink.handshakeSeconds)s; check `lf server status`")
                }
                throw RegistryQueryError(ServerLink.disconnected)
            }
            if received == 0 { ended = true; continue }
            buffer.append(contentsOf: chunk[..<received])
        }
    }
}

/// One subscription's frames; checkpoints before the connection is ready wait.
private final class ServerSubscription: @unchecked Sendable {
    let id = UUID().uuidString
    private let continuation: AsyncThrowingStream<Data, any Error>.Continuation
    private let lock = NSLock()
    private var link: ServerLink?
    /// The server coalesces confirmations: the newest reading answers earlier ones too.
    private var pending: UInt64?
    private var checkpoint: String?
    private var recovery: Task<Void, Never>?
    private let query: ServedRead
    private let connect: @Sendable () async throws -> ServerLink
    private var stopped = false

    init(_ continuation: AsyncThrowingStream<Data, any Error>.Continuation, query: ServedRead,
         connect: @escaping @Sendable () async throws -> ServerLink) {
        self.continuation = continuation
        self.query = query
        self.connect = connect
        continuation.onTermination = { [weak self] _ in self?.stop(nil) }
    }

    func start() async throws {
        try Task.checkCancellation()
        let link = try await connect()
        try Task.checkCancellation()
        try lock.withLock {
            guard !stopped else { return }
            try link.subscribe(self, query: query, resume: checkpoint)
            self.link = link
            if let pending {
                // A torn send shuts down the link; its failure resumes this watch.
                try? link.send(.checkpoint(id: id, checkpoint: pending))
            }
        }
    }

    /// Keep queued readings in this same stream: the resume token never outruns
    /// the baseline its consumer receives. A new stream starts without a token.
    func disconnected(from link: ServerLink, error: any Error) {
        let resume = lock.withLock {
            guard !stopped, self.link === link else { return true }
            self.link = nil
            guard checkpoint != nil else { return false }
            recovery = Task { [self] in
                do {
                    try await Task.sleep(for: .milliseconds(100))
                    try await start()
                } catch is CancellationError { }
                catch { finish(throwing: error) }
            }
            return true
        }
        if !resume { finish(throwing: error) }
    }

    /// A subscription's stdin line, `{"id": n}`: a checkpoint. A subscription's
    /// arguments are fixed; new arguments are a new subscription.
    func send(_ line: Data) {
        let checkpoint: UInt64
        do { checkpoint = try JSONDecoder().decode(Checkpoint.self, from: line).id } catch {
            finish(throwing: RegistryQueryError("Invalid subscription request: \(error.localizedDescription)"))
            return
        }
        let target: ServerLink? = lock.withLock {
            guard !stopped else { return nil }
            pending = checkpoint
            return link
        }
        guard let target else { return }
        // Send failure shuts down the link. Its disconnect callback retains this
        // unanswered confirmation and resumes it with the watch.
        try? target.send(.checkpoint(id: id, checkpoint: checkpoint))
    }

    private struct Checkpoint: Decodable {
        let id: UInt64
        init(from decoder: Decoder) throws {
            let box = try decoder.container(keyedBy: Keys.self)
            guard box.allKeys == [.id] else {
                throw DecodingError.dataCorrupted(.init(codingPath: [], debugDescription: "a subscription takes only checkpoints"))
            }
            id = try box.decode(UInt64.self, forKey: .id)
        }
        private enum Keys: String, CodingKey { case id, args }
    }

    /// From the reader thread: one complete reading.
    func receive(_ frame: JSONValue, checkpoint: String) {
        do {
            let line = try JSONEncoder().encode(frame)
            let delivered = lock.withLock {
                guard !stopped else { return true }
                switch continuation.yield(line) {
                case .enqueued:
                    self.checkpoint = checkpoint
                    if case .object(let fields) = frame, case .number(let answers) = fields["answers"],
                       pending.map(Double.init) == answers {
                        pending = nil
                    }
                    return true
                case .terminated: return true
                case .dropped: return false
                @unknown default: return false
                }
            }
            if !delivered { finish(throwing: RegistryQueryError("Server subscription fell behind; reconnect for current Work")) }
        } catch { finish(throwing: error) }
    }

    /// The server ended this subscription, or the connection failed.
    func finish(throwing error: any Error) { stop(error) }

    /// The server sent this subscription's last reading.
    func end() { stop(nil) }

    func cancel() async { stop(nil) }

    /// The first end wins; the server hears `cancel` once and the stream ends once.
    private func stop(_ error: (any Error)?) {
        let (first, link): (Bool, ServerLink?) = lock.withLock {
            defer { stopped = true; recovery?.cancel(); recovery = nil }
            return (!stopped, self.link)
        }
        guard first else { return }
        link?.unsubscribe(id)
        continuation.finish(throwing: error)
    }
}
#endif
