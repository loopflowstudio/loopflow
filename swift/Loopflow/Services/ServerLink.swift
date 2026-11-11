import Foundation

/// How one connection's messages travel: a Unix socket on the Machine, a
/// WebSocket over Tailscale. A carrier moves whole messages and knows nothing
/// of what they say.
public protocol ServerCarrier: Sendable {
    /// Sends one whole message. A failed send leaves the carrier closed.
    func send(_ message: Data) async throws
    /// The next whole message; nil once the peer closed between messages.
    func receive() async throws -> Data?
    /// Ends the connection; a pending `send` or `receive` fails.
    func close() async
}

/// Where a client reaches one server, and who it says it is.
public struct ServerEndpoint: Sendable {
    public let client: ServerClientIdentity
    /// Opens the control connection.
    public let control: @Sendable () async throws -> any ServerCarrier
    /// Opens a connection that presents one ticket.
    public let ticketed: @Sendable () async throws -> any ServerCarrier

    public init(client: ServerClientIdentity,
                control: @escaping @Sendable () async throws -> any ServerCarrier,
                ticketed: @escaping @Sendable () async throws -> any ServerCarrier) {
        self.client = client
        self.control = control
        self.ticketed = ticketed
    }
}

/// One server connection: hello and welcome once, then requests, subscriptions
/// and cancels by id. One reader demultiplexes answers. Losing the connection
/// fails pending requests; watches resume their retained stream on the next
/// link. Messages over the control limit travel on ticketed connections.
public actor ServerLink {
    public nonisolated let welcome: ServerWelcome
    private nonisolated let endpoint: ServerEndpoint
    private let carrier: any ServerCarrier
    private var order = ServerMessageOrder()
    private var pending: [String: CheckedContinuation<RawJSON, any Error>] = [:]
    private var subscriptions: [String: ServerSubscription] = [:]
    private var failure: (any Error)?
    private var unansweredReads = 0

    /// How long the server has to welcome a connection or admit a ticket.
    public static let handshake = Duration.seconds(10)
    /// How long one ticketed message may take to arrive.
    public static let transfer = Duration.seconds(60)
    /// How long an admission refusal is retried before it is reported.
    public static let busyWait = Duration.seconds(60)
    /// Bounded requests in a row that went unanswered before the link counts as stalled.
    public static let stalledReads = 3
    /// A lost connection: requests in flight have unknown outcomes, not failures.
    public static let disconnected = "Server disconnected; refresh state before retrying a command"
    public static let closed = "Server connection closed"
    public static let unanswered = "Loopflow server did not answer in time; check `lf server status`"
    static let handshakeExpired = "Loopflow server did not answer within 10s; check `lf server status`"

    public static func open(_ endpoint: ServerEndpoint) async throws -> ServerLink {
        let carrier = try await endpoint.control()
        do {
            let welcome = try await bounded(handshake, carrier, expired: handshakeExpired) {
                try await carrier.send(try line(.hello(ServerHello(
                    revision: ServerProtocol.revision, minRevision: ServerProtocol.minRevision, client: endpoint.client))))
                guard let first = try await carrier.receive() else {
                    throw RegistryQueryError("Loopflow server closed the connection without answering; it may speak an older protocol; update the selected installation")
                }
                switch try ServerMessage(line: first) {
                case .welcome(let welcome): return welcome
                case .error(_, let code, let message, let data): throw ServerError(code: code, message: message, data: data)
                default: throw RegistryQueryError("Server did not welcome the connection")
                }
            }
            let link = ServerLink(endpoint: endpoint, carrier: carrier, welcome: welcome)
            await link.read()
            return link
        } catch {
            await carrier.close()
            throw error
        }
    }

    private init(endpoint: ServerEndpoint, carrier: any ServerCarrier, welcome: ServerWelcome) {
        self.endpoint = endpoint
        self.carrier = carrier
        self.welcome = welcome
    }

    public var isClosed: Bool { failure != nil }

    /// Ends the connection on purpose: requests fail and watches end rather than resume.
    public func close() async {
        fail(RegistryQueryError(Self.closed), resumable: false)
    }

    // MARK: Requests

    /// What `operation` returned. With a `deadline`, an unanswered request is
    /// cancelled and reported; `stalledReads` in a row end the connection, so a
    /// stalled server shows as unavailable rather than current. Without one the
    /// request waits, because its outcome would otherwise become unknown.
    public func request(_ operation: ServerOperation, deadline: Duration? = nil) async throws -> RawJSON {
        guard let deadline else { return try await admitted(operation) }
        struct Expired: Error {}
        do {
            let value = try await withThrowingTaskGroup(of: RawJSON.self) { group in
                group.addTask { try await self.admitted(operation) }
                group.addTask {
                    try await Task.sleep(for: deadline)
                    throw Expired()
                }
                defer { group.cancelAll() }
                return try await group.next()!
            }
            unansweredReads = 0
            return value
        } catch is Expired {
            unansweredReads += 1
            if unansweredReads >= Self.stalledReads { await carrier.close() }
            throw RegistryQueryError(Self.unanswered)
        }
    }

    /// Admission refusals ran nothing. Give ordinary concurrent loading time to
    /// drain without replaying a dispatched or disconnected command.
    private func admitted(_ operation: ServerOperation) async throws -> RawJSON {
        let deadline = ContinuousClock.now + Self.busyWait
        var delay = Duration.milliseconds(100)
        while true {
            do { return try await requestOnce(operation) }
            catch let error as ServerError {
                guard ContinuousClock.now < deadline, error.code == .busy,
                      case .integer = error.data["max"],
                      case .string(let limit) = error.data["limit"],
                      ["open", "writes", "retained_bytes"].contains(limit) else { throw error }
                try await Task.sleep(for: min(delay, ContinuousClock.now.duration(to: deadline)))
                delay = min(delay * 2, .milliseconds(1600))
            }
        }
    }

    private struct Ticket: Decodable { let ticket: String }

    private func requestOnce(_ operation: ServerOperation) async throws -> RawJSON {
        let id = UUID().uuidString
        let message = try Self.line(.request(id: id, operation: operation, select: nil))
        if message.count <= ServerProtocol.maxMessage {
            return try await answer(id) { try await self.send(message) }
        }
        // An upload ticket admits a second connection carrying this one request;
        // its answer arrives here under the request's own id.
        let ticket = try await requestOnce(.requestUpload(bytes: message.count)).decode(Ticket.self).ticket
        return try await answer(id) {
            try await self.ticketed(ticket) { try await $0.send(message) }
        }
    }

    /// Cancelling sends `cancel` and releases the wait; a command already
    /// dispatched may still complete.
    private func answer(_ id: String, deliver: @escaping @Sendable () async throws -> Void) async throws -> RawJSON {
        try await withTaskCancellationHandler {
            try await withCheckedThrowingContinuation { continuation in
                if let failure { continuation.resume(throwing: failure); return }
                if Task.isCancelled { continuation.resume(throwing: CancellationError()); return }
                pending[id] = continuation
                Task {
                    do { try await deliver() } catch { await self.settle(id, throwing: error) }
                }
            }
        } onCancel: {
            Task { await self.abandon(id) }
        }
    }

    private func abandon(_ id: String) {
        order.discard(id)
        guard let waiting = pending.removeValue(forKey: id) else { return }
        Task { try? await self.send(Self.line(.cancel(id: id))) }
        waiting.resume(throwing: CancellationError())
    }

    /// Fail the request or subscription `id` without touching the connection.
    private func settle(_ id: String, throwing error: any Error) {
        order.discard(id)
        if let waiting = pending.removeValue(forKey: id) { waiting.resume(throwing: error); return }
        subscriptions.removeValue(forKey: id)?.finish(throwing: error)
    }

    // MARK: Subscriptions

    /// `query`'s readings, and its events when `events`, mapped by `map`; nil
    /// skips an update. After a reading, connection loss resumes the same stream
    /// on the link `connect` returns. `checkpoint(n)` asks for a reading whose
    /// `answers` is `n`; `cancel` ends it.
    public nonisolated static func subscribe<Output: Sendable>(
        _ query: ServedRead, events: Bool = false, buffer: Int = 8,
        connect: @escaping @Sendable () async throws -> ServerLink,
        map: @escaping @Sendable (ServerSubscriptionUpdate) throws -> Output?
    ) -> ServerWatch<Output> {
        let (updates, continuation) = AsyncThrowingStream<Output, any Error>.makeStream(bufferingPolicy: .bufferingOldest(buffer))
        let subscription = ServerSubscription(continuation, query: query, events: events, encode: map, connect: connect)
        Task {
            do { try await subscription.start() }
            catch { subscription.finish(throwing: error) }
        }
        return ServerWatch(updates: updates, checkpoint: subscription.checkpoint, cancel: subscription.cancel)
    }

    fileprivate func register(_ subscription: ServerSubscription, query: ServedRead, resume: String?) async throws {
        if let failure { throw failure }
        subscriptions[subscription.id] = subscription
        do { try await send(Self.line(.subscribe(id: subscription.id, operation: query, select: nil, resume: resume))) } catch {
            subscriptions.removeValue(forKey: subscription.id)
            throw error
        }
    }

    fileprivate func checkpoint(_ id: String, _ checkpoint: UInt64) async throws {
        if let failure { throw failure }
        try await send(Self.line(.checkpoint(id: id, checkpoint: checkpoint)))
    }

    fileprivate func unsubscribe(_ id: String) async {
        order.discard(id)
        guard subscriptions.removeValue(forKey: id) != nil, failure == nil else { return }
        try? await send(Self.line(.cancel(id: id)))
    }

    // MARK: Connection

    /// A torn message would corrupt whatever follows it: a failed send ends the connection.
    private func send(_ message: Data) async throws {
        if let failure { throw failure }
        do { try await carrier.send(message) } catch {
            await carrier.close()
            throw error
        }
    }

    private func read() {
        Task { [carrier] in
            let ended: any Error
            do {
                while let message = try await carrier.receive() { try await self.dispatch(message) }
                ended = RegistryQueryError(Self.disconnected)
            } catch { ended = error }
            await self.fail(ended, resumable: true)
        }
    }

    private func dispatch(_ line: Data) throws {
        guard failure == nil else { return }
        do {
            if let message = try order.receive(line) { try deliver(message) }
        } catch is ServerEventSequenceGap {
            for subscription in subscriptions.values { subscription.eventGap() }
        }
    }

    private func deliver(_ message: ServerMessage) throws {
        switch message {
        case .response(let id, _, let result):
            pending.removeValue(forKey: id)?.resume(returning: result)
            // A null result confirms a registered subscription; anything else is
            // a late answer to a forgotten request.
        case .reading(let id, let checkpoint, let frame):
            subscriptions[id]?.receive(frame, checkpoint: checkpoint)
        case .bulk(let id, let ticket, _):
            // Fetched off the reader, so other answers never wait behind it.
            Task {
                do {
                    let payload = try await ticketed(ticket) { carrier in
                        guard let message = try await carrier.receive() else {
                            throw RegistryQueryError("Server closed the bulk connection without its message")
                        }
                        return message
                    }
                    try await completeBulk(id, payload: payload)
                } catch { settle(id, throwing: error) }
            }
        case .event(let id, _, let event):
            subscriptions[id]?.receive(event)
        case .ended(let id):
            subscriptions.removeValue(forKey: id)?.end()
        case .error(let id?, let code, let message, let data):
            settle(id, throwing: ServerError(code: code, message: message, data: data))
        case .error(nil, let code, let message, let data):
            throw ServerError(code: code, message: message, data: data)
        case .welcome, .status, .bridged, .admitted:
            throw RegistryQueryError("Server repeated its greeting; update the client")
        }
    }

    private func completeBulk(_ id: String, payload: Data) throws {
        guard failure == nil, let (message, queued) = try order.complete(id, payload: payload) else { return }
        try deliver(message)
        for (message, size) in queued {
            if let next = try order.resume(message, bytes: size) { try deliver(next) }
        }
    }

    /// A ticketed connection: present `ticket`, read `admitted`, then `body` has
    /// the connection to itself.
    private nonisolated func ticketed<T: Sendable>(_ ticket: String, _ body: @Sendable (any ServerCarrier) async throws -> T) async throws -> T {
        let carrier = try await endpoint.ticketed()
        do {
            try await Self.bounded(Self.handshake, carrier, expired: Self.handshakeExpired) {
                try await carrier.send(Self.line(.attach(ticket: ticket)))
                guard let first = try await carrier.receive() else {
                    throw RegistryQueryError("Server closed the ticketed connection without admitting it")
                }
                switch try ServerMessage(line: first) {
                case .admitted: break
                case .error(_, let code, let message, let data): throw ServerError(code: code, message: message, data: data)
                default: throw RegistryQueryError("Server did not admit the ticket; update the client")
                }
            }
            let value = try await Self.bounded(Self.transfer, carrier, expired: Self.unanswered) { try await body(carrier) }
            await carrier.close()
            return value
        } catch {
            await carrier.close()
            throw error
        }
    }

    private func fail(_ error: any Error, resumable: Bool) {
        guard failure == nil else { return }
        failure = error
        order = ServerMessageOrder()
        let (waiting, watching) = (pending.values, subscriptions.values)
        pending = [:]
        subscriptions = [:]
        for continuation in waiting { continuation.resume(throwing: RegistryQueryError(Self.disconnected)) }
        for subscription in watching {
            if resumable { subscription.disconnected(from: self, error: error) } else { subscription.finish(throwing: error) }
        }
        Task { [carrier] in await carrier.close() }
    }

    /// `body` within `limit`; on expiry the carrier closes and the failure reads as a timeout.
    static func bounded<T: Sendable>(_ limit: Duration, _ carrier: any ServerCarrier, expired message: String,
                                     _ body: () async throws -> T) async throws -> T {
        let expired = Expiry()
        let timer = Task {
            try await Task.sleep(for: limit)
            expired.fire()
            await carrier.close()
        }
        defer { timer.cancel() }
        do { return try await body() } catch {
            if expired.fired { throw RegistryQueryError(message) }
            throw error
        }
    }

    static func line(_ message: ServerClientMessage) throws -> Data {
        let line = try JSONEncoder().encode(message)
        guard line.count <= ServerProtocol.maxBulk else { throw RegistryQueryError("Server request exceeds its limit") }
        return line
    }
}

private final class Expiry: @unchecked Sendable {
    private let lock = NSLock()
    private var value = false
    var fired: Bool { lock.withLock { value } }
    func fire() { lock.withLock { value = true } }
}

extension RegistryQuery {
    /// Typed reads and writes over the server `link` returns; nothing runs `lf`.
    /// A read or write unanswered within `deadline` is cancelled and reported.
    public static func served(deadline: Duration?, link: @escaping @Sendable () async throws -> ServerLink) -> RegistryQuery {
        RegistryQuery(serve: { read in
            try await link().request(.read(read), deadline: deadline).printed
        }, write: { change in
            String(decoding: try await link().request(.command(change), deadline: deadline).bytes, as: UTF8.self)
        }) { args, _ in
            throw RegistryQueryError("`lf \(args.joined(separator: " "))` runs only on the Machine")
        }
    }
}

public enum ServerSubscriptionUpdate: Sendable {
    case reading(RawJSON)
    case event(SubscriptionEvent)
}

/// A live subscription's updates and its two controls.
public struct ServerWatch<Output: Sendable>: Sendable {
    public let updates: AsyncThrowingStream<Output, any Error>
    public let checkpoint: @Sendable (UInt64) -> Void
    public let cancel: @Sendable () -> Void
}

/// One subscription's updates across links; checkpoints before a link is ready wait.
final class ServerSubscription: @unchecked Sendable {
    let id = UUID().uuidString
    private let deliver: @Sendable (ServerSubscriptionUpdate) throws -> Bool
    private let finishStream: @Sendable ((any Error)?) -> Void
    private let lock = NSLock()
    private var link: ServerLink?
    /// The server coalesces confirmations: the newest reading answers earlier ones too.
    private var pending: UInt64?
    private var resumeToken: String?
    private var eventBaseline = false
    private var recovery: Task<Void, Never>?
    private let query: ServedRead
    private let consumesEvents: Bool
    private let connect: @Sendable () async throws -> ServerLink
    private var stopped = false

    init<Output: Sendable>(_ continuation: AsyncThrowingStream<Output, any Error>.Continuation,
                           query: ServedRead, events: Bool,
                           encode: @escaping @Sendable (ServerSubscriptionUpdate) throws -> Output?,
                           connect: @escaping @Sendable () async throws -> ServerLink) {
        self.query = query
        self.consumesEvents = events
        self.connect = connect
        self.deliver = { update in
            guard let output = try encode(update) else { return true }
            switch continuation.yield(output) {
            case .enqueued, .terminated: return true
            case .dropped: return false
            @unknown default: return false
            }
        }
        self.finishStream = { continuation.finish(throwing: $0) }
        continuation.onTermination = { [weak self] _ in self?.stop(nil) }
    }

    func start() async throws {
        try Task.checkCancellation()
        let link = try await connect()
        try Task.checkCancellation()
        let (stopped, resume) = lock.withLock { (self.stopped, consumesEvents ? nil : resumeToken) }
        guard !stopped else { return }
        try await link.register(self, query: query, resume: resume)
        let checkpoint: UInt64? = lock.withLock {
            self.link = link
            eventBaseline = false
            return pending
        }
        // A failed send ends the link; its failure resumes this watch.
        if let checkpoint { try? await link.checkpoint(id, checkpoint) }
    }

    /// Keep queued readings in this same stream: the resume token never outruns
    /// the baseline its consumer receives. A new stream starts without a token.
    func disconnected(from link: ServerLink, error: any Error) {
        let resume = lock.withLock {
            guard !stopped, self.link === link else { return true }
            self.link = nil
            guard resumeToken != nil else { return false }
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

    func checkpoint(_ checkpoint: UInt64) {
        let target: ServerLink? = lock.withLock {
            guard !stopped else { return nil }
            pending = checkpoint
            return link
        }
        guard let target else { return }
        // A failed send ends the link. Its disconnect retains this unanswered
        // confirmation and resumes it with the watch.
        Task { [id] in try? await target.checkpoint(id, checkpoint) }
    }

    /// From the link's reader: one complete reading, ordered before its events.
    func receive(_ frame: RawJSON, checkpoint: String) {
        do {
            let delivered = try lock.withLock {
                guard !stopped else { return true }
                guard try deliver(.reading(frame)) else { return false }
                resumeToken = checkpoint
                eventBaseline = true
                if let pending, try frame.fields()["answers"]?.decode(UInt64?.self) == pending {
                    self.pending = nil
                }
                return true
            }
            if !delivered { finish(throwing: RegistryQueryError("Server subscription fell behind; reconnect for current Work")) }
        } catch { finish(throwing: error) }
    }

    func receive(_ event: SubscriptionEvent) {
        guard consumesEvents else { return }
        if case .unknown(let kind) = event {
            finish(throwing: RegistryQueryError("Unknown subscription event \(kind); subscribe again for current Work"))
            return
        }
        do {
            let delivered = try lock.withLock {
                guard !stopped, eventBaseline else { return true }
                return try deliver(.event(event))
            }
            if !delivered { finish(throwing: RegistryQueryError("Server event consumer fell behind; subscribe again for current Work")) }
        } catch { finish(throwing: error) }
    }

    func eventGap() {
        guard consumesEvents else { return }
        finish(throwing: RegistryQueryError("Server event sequence has a gap; subscribe again for current Work"))
    }

    /// The server ended this subscription, or the connection failed.
    func finish(throwing error: any Error) { stop(error) }

    /// The server sent this subscription's last reading.
    func end() { stop(nil) }

    func cancel() { stop(nil) }

    /// The first end wins; the server hears `cancel` once and the stream ends once.
    private func stop(_ error: (any Error)?) {
        let (first, link): (Bool, ServerLink?) = lock.withLock {
            defer { stopped = true; recovery?.cancel(); recovery = nil }
            return (!stopped, self.link)
        }
        guard first else { return }
        if let link { Task { [id] in await link.unsubscribe(id) } }
        finishStream(error)
    }
}
