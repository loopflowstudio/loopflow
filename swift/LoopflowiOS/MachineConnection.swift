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
final class MachineSocket: ServerCarrier {
    private static let session: URLSession = {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.httpShouldSetCookies = false
        configuration.urlCache = nil
        return URLSession(configuration: configuration)
    }()
    private let socket: URLSessionWebSocketTask
    private(set) var closed = false
    private var timedOut = false
    private var opened = false
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

    /// Sends one protocol message. The first message on a socket waits for TLS
    /// too, so it sends with no limit of its own inside the handshake's deadline.
    func send(_ message: Data) async throws {
        guard !closed else { throw connectionError(RegistryQueryError("Machine connection closed")) }
        let timeout = opened ? deadline(seconds: 5) : nil
        opened = true
        defer { timeout?.cancel() }
        do { try await socket.send(.string(String(decoding: message, as: UTF8.self))) } catch { throw connectionError(error) }
    }

    /// The next protocol message.
    func receive() async throws -> Data? { try await receiveText() }

    /// Sends terminal bytes.
    func sendBytes(_ bytes: Data) async throws {
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

extension MachineSocket {
    /// A Session's terminal: present `ticket`, read `bridged`; the socket then
    /// carries the owner protocol unchanged.
    static func terminal(endpoint: URL, ticket: String) async throws -> MachineSocket {
        let socket = try MachineSocket(endpoint: endpoint, path: ServerProtocol.ticketPath, maximumMessage: ServerProtocol.maxBulk)
        do {
            let handshake = socket.deadline(seconds: 10)
            defer { handshake.cancel() }
            try await socket.send(try JSONEncoder().encode(ServerClientMessage.attach(ticket: ticket)))
            switch try ServerMessage(line: try await socket.receiveText()) {
            case .bridged: return socket
            case .error(_, let code, let message, let data): throw ServerError(code: code, message: message, data: data)
            default: throw RegistryQueryError("The Machine did not attach the Session")
            }
        } catch { socket.close(); throw error }
    }
}

extension ServerEndpoint {
    /// The Machine's server over its Tailscale HTTPS address.
    static func machine(_ endpoint: URL) -> ServerEndpoint {
        let version = Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "dev"
        return ServerEndpoint(
            client: ServerClientIdentity(kind: "mobile", platform: "ios", name: "Loopflow", version: version),
            control: { @MainActor in
                let socket = try MachineSocket(endpoint: endpoint, path: ServerProtocol.machinePath, maximumMessage: ServerProtocol.maxMessage)
                // Foreground streams need a liveness check even when nothing changes.
                socket.keepAlive()
                return socket
            },
            ticketed: { @MainActor in
                try MachineSocket(endpoint: endpoint, path: ServerProtocol.ticketPath, maximumMessage: ServerProtocol.maxBulk)
            })
    }
}
