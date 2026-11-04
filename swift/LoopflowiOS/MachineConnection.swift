import Foundation
import Loopflow
import os

/// Local saved state, stored only in Keychain. Never include this value in logs.
struct MachineCredential: Codable {
    let endpoint: URL
    let deviceID: String
    let name: String
    let token: String
    // A newly issued grant is saved before its first bootstrap succeeds.
    let machineID: String?
}

/// One application operation per WebSocket. Message boundaries are unrelated
/// to the Machine/owner framing; all reads and writes are bounded.
@MainActor
final class MachineConnection {
    private static let session: URLSession = {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.httpShouldSetCookies = false
        configuration.urlCache = nil
        return URLSession(configuration: configuration)
    }()
    private let socket: URLSessionWebSocketTask
    private var buffered = Data()
    private var closed = false
    private var timedOut = false
    private var heartbeat: Task<Void, Never>?

    init(endpoint: URL, token: String, path: String = "/v1/machine") throws {
        let url = try Self.socketURL(endpoint: endpoint, path: path)
        var request = URLRequest(url: url, cachePolicy: .reloadIgnoringLocalCacheData, timeoutInterval: 15)
        request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        socket = Self.session.webSocketTask(with: request)
        socket.maximumMessageSize = 64 * 1024
        socket.resume()
    }

    static func socketURL(endpoint: URL, path: String) throws -> URL {
        try MachinePairingTicket.socketURL(endpoint: endpoint, path: path)
    }

    func close() {
        guard !closed else { return }
        closed = true
        heartbeat?.cancel()
        heartbeat = nil
        socket.cancel(with: .goingAway, reason: nil)
        buffered.removeAll()
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

    func deadline(seconds: Int = 15) -> Task<Void, Never> {
        Task { [weak self] in
            do {
                try await Task.sleep(for: .seconds(seconds))
                self?.timedOut = true
                self?.close()
            } catch { /* The operation completed or its owner canceled it. */ }
        }
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

    func request(_ operation: MachineOperation, id: String = UUID().uuidString) async throws -> MachineResponse {
        let timeout = deadline()
        defer { timeout.cancel() }
        let body = try JSONEncoder().encode(MachineRequest(version: 1, id: id, operation: operation))
        try await sendFrame(body)
        return try await receiveResponse(id: id)
    }

    func sendFrame(_ body: Data) async throws {
        guard body.count <= 65536 else { throw RegistryQueryError("Machine request exceeds its limit") }
        var length = UInt32(body.count).bigEndian
        var bytes = withUnsafeBytes(of: &length) { Data($0) }
        bytes.append(body)
        try await send(bytes)
    }

    func receiveResponse(id: String) async throws -> MachineResponse {
        let header = try await read(4)
        let size = header.reduce(0) { ($0 << 8) | Int($1) }
        guard size <= 32 * 1024 * 1024 else { throw RegistryQueryError("Machine reading exceeds its limit") }
        let response = try JSONDecoder().decode(MachineResponse.self, from: await read(size))
        guard response.id == id else { throw RegistryQueryError("Machine response belongs to another request") }
        if case .error(_, let message) = response.result { throw RegistryQueryError(message) }
        return response
    }

    func receiveBytes() async throws -> Data {
        if !buffered.isEmpty {
            let data = buffered
            buffered = Data()
            return data
        }
        return try await nextMessage()
    }

    private func read(_ count: Int) async throws -> Data {
        while buffered.count < count { buffered.append(try await nextMessage()) }
        let result = Data(buffered.prefix(count))
        buffered.removeFirst(count)
        return result
    }

    private func nextMessage() async throws -> Data {
        guard !closed else { throw connectionError(RegistryQueryError("Machine connection closed")) }
        do {
            return try await withTaskCancellationHandler {
                guard case .data(let data) = try await socket.receive() else {
                    throw RegistryQueryError("The Machine sent an unsupported message")
                }
                return data
            } onCancel: { [socket] in socket.cancel(with: .goingAway, reason: nil) }
        } catch { throw connectionError(error) }
    }

    private func connectionError(_ error: any Error) -> any Error {
        if Task.isCancelled { return CancellationError() }
        if timedOut { return RegistryQueryError("The Machine did not respond in time") }
        if closed { return RegistryQueryError("Machine connection closed") }
        return error
    }
}
