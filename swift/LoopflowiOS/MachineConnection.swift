import Foundation
import Loopflow
import os

/// The verified Machine selected on this phone. No network credential is stored.
struct SavedMachine: Codable {
    let endpoint: URL
    let machineID: String
}

/// One WebSocket to the Machine's server. Text frames carry protocol messages;
/// binary frames carry a terminal's bytes. All reads and writes are bounded.
@MainActor
final class MachineSocket {
    private static let session: URLSession = {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.httpShouldSetCookies = false
        configuration.urlCache = nil
        return URLSession(configuration: configuration)
    }()
    private let socket: URLSessionWebSocketTask
    private(set) var closed = false
    private var timedOut = false
    private var heartbeat: Task<Void, Never>?

    init(endpoint: URL, path: String, maximumMessage: Int) throws {
        let url = try MachineAddress.socketURL(endpoint: endpoint, path: path)
        let request = URLRequest(url: url, cachePolicy: .reloadIgnoringLocalCacheData, timeoutInterval: 15)
        socket = Self.session.webSocketTask(with: request)
        socket.maximumMessageSize = maximumMessage
        socket.resume()
    }

    func close() {
        guard !closed else { return }
        closed = true
        heartbeat?.cancel()
        heartbeat = nil
        socket.cancel(with: .goingAway, reason: nil)
    }

    /// Long-lived foreground streams need a bounded liveness check even when
    /// no Work or terminal output changes. This never resends application data.
    func keepAlive() {
        guard heartbeat == nil, !closed else { return }
        heartbeat = Task { [weak self] in
            while !Task.isCancelled {
                do { try await Task.sleep(for: .seconds(20)) } catch { return }
                guard let self, !closed else { return }
                ping()
            }
        }
    }

    private func ping() {
        let timeout = deadline(seconds: 5)
        // Foundation can report a pending ping's failure again when the socket
        // is canceled. Settle once without bridging that callback to a continuation.
        let completed = OSAllocatedUnfairLock(initialState: false)
        socket.sendPing { [weak self] error in
            let first = completed.withLock { settled in
                guard !settled else { return false }
                settled = true
                return true
            }
            guard first else { return }
            timeout.cancel()
            if error != nil { Task { @MainActor in self?.close() } }
        }
    }

    /// Closes the socket unless cancelled first; its errors then read as a timeout.
    func deadline(seconds: Int = 15) -> Task<Void, Never> {
        Task { [weak self] in
            do {
                try await Task.sleep(for: .seconds(seconds))
                self?.timedOut = true
                self?.close()
            } catch { /* The operation completed or its owner canceled it. */ }
        }
    }

    func send(_ text: String) async throws {
        guard !closed else { throw connectionError(RegistryQueryError("Machine connection closed")) }
        let timeout = deadline(seconds: 5)
        defer { timeout.cancel() }
        do { try await socket.send(.string(text)) } catch { throw connectionError(error) }
    }

    func send(_ bytes: Data) async throws {
        guard !closed else { throw connectionError(RegistryQueryError("Machine connection closed")) }
        let timeout = deadline(seconds: 5)
        defer { timeout.cancel() }
        do {
            var index = bytes.startIndex
            while index < bytes.endIndex {
                try Task.checkCancellation()
                let end = min(index + 64 * 1024, bytes.endIndex)
                try await socket.send(.data(Data(bytes[index..<end])))
                index = end
            }
        } catch { throw connectionError(error) }
    }

    /// The next protocol message.
    func receiveText() async throws -> Data {
        guard case .string(let text) = try await next() else {
            throw connectionError(RegistryQueryError("The Machine sent an unsupported message"))
        }
        return Data(text.utf8)
    }

    /// The next terminal bytes.
    func receiveBytes() async throws -> Data {
        guard case .data(let bytes) = try await next() else {
            throw connectionError(RegistryQueryError("The Machine sent an unsupported message"))
        }
        return bytes
    }

    private func next() async throws -> URLSessionWebSocketTask.Message {
        guard !closed else { throw connectionError(RegistryQueryError("Machine connection closed")) }
        do {
            return try await withTaskCancellationHandler {
                try await socket.receive()
            } onCancel: { [socket] in socket.cancel(with: .goingAway, reason: nil) }
        } catch { throw connectionError(error) }
    }

    private func connectionError(_ error: any Error) -> any Error {
        if Task.isCancelled { return CancellationError() }
        if timedOut { return RegistryQueryError("The Machine did not respond in time") }
        if let response = socket.response as? HTTPURLResponse, response.statusCode == 403 {
            #if targetEnvironment(simulator)
            return RegistryQueryError("Connect from a different device signed in to the Machine owner’s Tailscale account. A Simulator on the Machine itself shares its identity and cannot use this connection.")
            #else
            return RegistryQueryError("Connect with the Machine owner’s Tailscale account, then try again.")
            #endif
        }
        if closed { return RegistryQueryError("Machine connection closed") }
        return error
    }
}

/// The control connection: hello and welcome once, then requests, subscriptions
/// and cancels by id. One reader demultiplexes answers. Messages over the
/// control limit travel on ticketed connections, as does a terminal's bytes.
@MainActor
final class MachineConnection {
    let welcome: ServerWelcome
    private let endpoint: URL
    private let socket: MachineSocket
    private var pending: [String: PendingRequest] = [:]
    private var subscriptions: [String: MachineSubscription] = [:]
    private var failure: (any Error)?
    private var reader: Task<Void, Never>?

    private struct PendingRequest {
        let continuation: CheckedContinuation<JSONValue, any Error>
        let timeout: Task<Void, Never>
    }

    static func open(endpoint: URL) async throws -> MachineConnection {
        let socket = try MachineSocket(endpoint: endpoint, path: ServerProtocol.machinePath, maximumMessage: ServerProtocol.maxMessage)
        do {
            let handshake = socket.deadline(seconds: 10)
            defer { handshake.cancel() }
            let hello = ServerHello(revision: ServerProtocol.revision, minRevision: ServerProtocol.minRevision, client: ServerClientIdentity(
                kind: "mobile", platform: "ios", name: "Loopflow",
                version: Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "dev"))
            try await socket.send(try line(.hello(hello)))
            switch try JSONDecoder().decode(ServerMessage.self, from: try await socket.receiveText()) {
            case .welcome(let welcome): return MachineConnection(endpoint: endpoint, socket: socket, welcome: welcome)
            case .error(_, let code, let message, let data): throw ServerError(code: code, message: message, data: data)
            default: throw RegistryQueryError("The Machine did not welcome the connection")
            }
        } catch { socket.close(); throw error }
    }

    private init(endpoint: URL, socket: MachineSocket, welcome: ServerWelcome) {
        self.endpoint = endpoint
        self.socket = socket
        self.welcome = welcome
        socket.keepAlive()
        reader = Task { [weak self] in
            do {
                while let self, !Task.isCancelled { try self.dispatch(try await socket.receiveText()) }
            } catch { self?.fail(error) }
        }
    }

    var isClosed: Bool { failure != nil }

    func close() { fail(RegistryQueryError("Machine connection closed")) }

    /// How long a request may wait for its answer.
    static let requestSeconds = 15

    /// One request and its answer. A timeout or cancellation sends `cancel` and
    /// releases the wait; a dispatched command still completes.
    func request(_ operation: ServerOperation) async throws -> JSONValue {
        let id = UUID().uuidString
        let line = try Self.line(.request(id: id, operation: operation, select: nil))
        return try await withTaskCancellationHandler {
            try await withCheckedThrowingContinuation { continuation in
                if let failure { continuation.resume(throwing: failure); return }
                let timeout = Task { [weak self] in
                    do { try await Task.sleep(for: .seconds(Self.requestSeconds)) } catch { return }
                    self?.forget(id, throwing: RegistryQueryError("The Machine did not respond in time"))
                }
                pending[id] = PendingRequest(continuation: continuation, timeout: timeout)
                Task { [weak self] in
                    do {
                        if line.utf8.count <= ServerProtocol.maxMessage { try await self?.socket.send(line) }
                        else { try await self?.upload(line) }
                    } catch { self?.forget(id, throwing: error) }
                }
            }
        } onCancel: {
            Task { @MainActor [weak self] in self?.forget(id, throwing: CancellationError()) }
        }
    }

    /// A live subscription; its readings arrive until `cancel` or the server ends it.
    func subscribe(_ query: ServedRead) async throws -> MachineSubscription {
        let subscription = MachineSubscription(connection: self)
        subscriptions[subscription.id] = subscription
        do {
            try await socket.send(try Self.line(.subscribe(id: subscription.id, operation: query, select: nil, resume: nil)))
        } catch {
            subscriptions.removeValue(forKey: subscription.id)
            throw error
        }
        return subscription
    }

    fileprivate func checkpoint(_ id: String, _ checkpoint: UInt64) async throws {
        if let failure { throw failure }
        try await socket.send(try Self.line(.checkpoint(id: id, checkpoint: checkpoint)))
    }

    fileprivate func unsubscribe(_ id: String) {
        guard subscriptions.removeValue(forKey: id) != nil, failure == nil else { return }
        Task { try? await socket.send(try Self.line(.cancel(id: id))) }
    }

    /// Resolve the Session’s current owner and open its unchanged terminal protocol.
    func attach(session: String) async throws -> MachineSocket {
        guard case .string(let ticket) = try await request(.sessionAttach(session: session, owner: nil))["ticket"] else {
            throw RegistryQueryError("The Machine did not attach the Session")
        }
        return try await Self.redeem(endpoint: endpoint, ticket: ticket, terminal: true)
    }

    /// A ticketed connection, admitted. The caller owns it from here.
    private static func redeem(endpoint: URL, ticket: String, terminal: Bool = false) async throws -> MachineSocket {
        let socket = try MachineSocket(endpoint: endpoint, path: ServerProtocol.ticketPath, maximumMessage: ServerProtocol.maxBulk)
        do {
            let handshake = socket.deadline(seconds: 10)
            defer { handshake.cancel() }
            try await socket.send(try line(.attach(ticket: ticket)))
            switch try JSONDecoder().decode(ServerMessage.self, from: try await socket.receiveText()) {
            case .admitted where !terminal: return socket
            case .bridged where terminal: return socket
            case .error(_, let code, let message, let data): throw ServerError(code: code, message: message, data: data)
            default: throw RegistryQueryError("The Machine did not admit the ticket")
            }
        } catch { socket.close(); throw error }
    }

    /// One request too large for the control connection; its answer still
    /// arrives here under the request's id.
    private func upload(_ line: String) async throws {
        guard case .string(let ticket) = try await request(.requestUpload(bytes: line.utf8.count))["ticket"] else {
            throw RegistryQueryError("The Machine issued no upload ticket")
        }
        let bulk = try await Self.redeem(endpoint: endpoint, ticket: ticket)
        defer { bulk.close() }
        let timeout = bulk.deadline(seconds: 60)
        defer { timeout.cancel() }
        try await bulk.send(line)
    }

    /// Fetch a spilled message off the reader, so other readings never wait behind it.
    private func fetch(_ id: String, ticket: String) {
        let endpoint = endpoint
        Task { [weak self] in
            do {
                let bulk = try await Self.redeem(endpoint: endpoint, ticket: ticket)
                defer { bulk.close() }
                let timeout = bulk.deadline(seconds: 60)
                defer { timeout.cancel() }
                let message = try await bulk.receiveText()
                try self?.dispatch(message)
            } catch { self?.forget(id, throwing: error) }
        }
    }

    private func dispatch(_ line: Data) throws {
        switch try JSONDecoder().decode(ServerMessage.self, from: line) {
        case .response(let id, _, let result):
            if let waiting = pending.removeValue(forKey: id) {
                waiting.timeout.cancel()
                waiting.continuation.resume(returning: result)
            }
            // A null result confirms a registered subscription; anything else is a late answer.
        case .reading(let id, _, let frame):
            subscriptions[id]?.receive(frame)
        case .bulk(let id, let ticket, _):
            fetch(id, ticket: ticket)
        case .ended(let id):
            subscriptions.removeValue(forKey: id)?.end()
        case .error(let id?, let code, let message, let data):
            forget(id, throwing: ServerError(code: code, message: message, data: data))
        case .error(nil, let code, let message, let data):
            throw ServerError(code: code, message: message, data: data)
        case .welcome, .status, .admitted, .bridged:
            throw RegistryQueryError("The Machine repeated its greeting; update the app")
        }
    }

    /// Fail the request or subscription `id` without touching the connection.
    private func forget(_ id: String, throwing error: any Error) {
        if let waiting = pending.removeValue(forKey: id) {
            waiting.timeout.cancel()
            if failure == nil { Task { try? await socket.send(try Self.line(.cancel(id: id))) } }
            waiting.continuation.resume(throwing: error)
            return
        }
        if let subscription = subscriptions.removeValue(forKey: id) { subscription.finish(throwing: error) }
    }

    private func fail(_ error: any Error) {
        guard failure == nil else { return }
        failure = error
        reader?.cancel()
        let waiting = pending.values
        let watching = subscriptions.values
        pending = [:]
        subscriptions = [:]
        // Outcomes of requests in flight are unknown, not failed.
        for request in waiting {
            request.timeout.cancel()
            request.continuation.resume(throwing: RegistryQueryError("Machine connection closed; refresh before retrying"))
        }
        for subscription in watching { subscription.finish(throwing: error) }
        socket.close()
    }

    private static func line(_ message: ServerClientMessage) throws -> String {
        let line = try JSONEncoder().encode(message)
        guard line.count <= ServerProtocol.maxBulk else { throw RegistryQueryError("Machine request exceeds its limit") }
        return String(decoding: line, as: UTF8.self)
    }
}

/// One subscription's readings in order; `checkpoint` asks for a reading whose
/// `answers` carries the checkpoint, `cancel` ends it.
@MainActor
final class MachineSubscription {
    let id = UUID().uuidString
    let readings: AsyncThrowingStream<JSONValue, any Error>
    private let continuation: AsyncThrowingStream<JSONValue, any Error>.Continuation
    private weak var connection: MachineConnection?
    private var ended = false

    fileprivate init(connection: MachineConnection) {
        self.connection = connection
        (readings, continuation) = AsyncThrowingStream.makeStream()
    }

    func checkpoint(_ checkpoint: UInt64) async throws {
        guard let connection, !ended else { throw RegistryQueryError("Task updates disconnected; reconnect for current Tasks") }
        try await connection.checkpoint(id, checkpoint)
    }

    func cancel() {
        guard !ended else { return }
        ended = true
        connection?.unsubscribe(id)
        continuation.finish()
    }

    fileprivate func receive(_ frame: JSONValue) {
        continuation.yield(frame)
    }

    fileprivate func end() {
        ended = true
        continuation.finish()
    }

    fileprivate func finish(throwing error: any Error) {
        guard !ended else { return }
        ended = true
        continuation.finish(throwing: error)
    }
}
