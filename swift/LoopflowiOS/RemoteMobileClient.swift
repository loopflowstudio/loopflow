import Foundation
import Loopflow

@MainActor
final class RemoteMobileClient: MobileClient {
    private(set) var credential: MachineCredential

    init(credential: MachineCredential) { self.credential = credential }

    func workingTasks() async throws -> [WorkViewPlan.Content] {
        try await verifyMachine()
        let output = try await read(Self.taskQuery)
        return try JSONDecoder().decode([WorkViewPlan].self, from: Data(output.utf8)).flatMap { try mobileTasks($0) }
    }

    private static let taskQuery = ServedRead.allViewPlans(expression: "kind:task state:open")

    func observeTasks() async throws -> any MobileTaskObservation {
        try await verifyMachine()
        return try await RemoteTaskObservation.open(credential: credential, query: Self.taskQuery)
    }

    func attach(sessionID: String) async throws -> any MobileAttachment {
        try await verifyMachine()
        let response = try await response(.sessionOwner(session: sessionID))
        guard case .sessionOwner(let owner) = response.result else { throw RegistryQueryError("The Machine did not resolve the Session owner") }
        guard let owner else { throw SessionUnavailable("This Session has no running owner. Open it on its Machine first.") }
        let connection = try MachineConnection(endpoint: credential.endpoint, token: credential.token)
        do {
            let response = try await connection.request(.attach(owner: owner))
            guard case .bridged = response.result else { throw RegistryQueryError("The Machine did not attach the Session") }
            return try await RemoteAttachment.open(sessionID: sessionID, connection: connection)
        } catch { connection.close(); throw error }
    }

    private func verifyMachine() async throws {
        let reply = try await response(.bootstrap)
        guard case .bootstrap(let version, _, _, let machine, let capabilities, _, _) = reply.result,
              version == 1, case .object(let fields) = machine,
              case .string(let machineID) = fields["id"] else {
            throw RegistryQueryError("The Machine did not return its identity")
        }
        if let pairedID = credential.machineID, pairedID != machineID {
            throw RegistryQueryError("This address no longer identifies the paired Machine. Pair it again before continuing.")
        }
        if credential.machineID == nil {
            let confirmed = MachineCredential(endpoint: credential.endpoint, deviceID: credential.deviceID,
                name: credential.name, token: credential.token, machineID: machineID)
            try MachineKeychain.save(confirmed)
            credential = confirmed
        }
        guard ["all_view_plans", "session_owner", "owner_bridge"].allSatisfy(capabilities.contains) else {
            throw RegistryQueryError("Update the Machine service to use Tasks and Sessions from this phone")
        }
    }

    private func read(_ query: ServedRead) async throws -> String {
        guard case .read(let output) = try await response(.read(query)).result else {
            throw RegistryQueryError("The Machine did not return a reading")
        }
        return output
    }

    private func response(_ operation: MachineOperation) async throws -> MachineResponse {
        let connection = try MachineConnection(endpoint: credential.endpoint, token: credential.token)
        defer { connection.close() }
        return try await connection.request(operation)
    }
}

@MainActor
final class RemoteAttachment: MobileAttachment {
    let sessionID: String
    let events: AsyncStream<MobileTerminalEvent>
    private let continuation: AsyncStream<MobileTerminalEvent>.Continuation
    private let connection: MachineConnection
    private var decoder = OwnerStream()
    private var reader: Task<Void, Never>?
    private var writer: Task<Void, Never>?
    private let writes: AsyncStream<Data>
    private let outgoing: AsyncStream<Data>.Continuation
    private var pendingBytes = 0
    private var nextRequest: UInt64 = 1
    private var closed = false
    private var controlDeadline: Task<Void, Never>?

    private init(sessionID: String, connection: MachineConnection) {
        self.sessionID = sessionID
        self.connection = connection
        (events, continuation) = AsyncStream.makeStream(bufferingPolicy: .bufferingOldest(16))
        (writes, outgoing) = AsyncStream.makeStream(bufferingPolicy: .bufferingOldest(64))
    }

    static func open(sessionID: String, connection: MachineConnection) async throws -> RemoteAttachment {
        let link = RemoteAttachment(sessionID: sessionID, connection: connection)
        let timeout = connection.deadline(seconds: 5)
        defer { timeout.cancel() }
        link.decoder.expectAttachment(1)
        try await connection.send(link.attachRequest(id: 1, control: false, size: TerminalGeometry(columns: 80, rows: 24)))
        while !link.decoder.ready && !link.decoder.ended {
            for event in try link.decoder.receive(await connection.receiveBytes()) { try link.emit(event) }
        }
        link.start()
        return link
    }

    func requestControl(size: TerminalGeometry) throws {
        guard !closed, decoder.ready else { throw RegistryQueryError("The Session is not ready for keyboard input") }
        nextRequest += 1
        decoder.expectAttachment(nextRequest)
        controlDeadline = connection.deadline(seconds: 5)
        try enqueue(attachRequest(id: nextRequest, control: true, size: size))
    }

    func send(_ bytes: Data) throws {
        guard !closed, decoder.ready, decoder.inputControl else { throw RegistryQueryError("Keyboard control was released") }
        try enqueue(OwnerStream.frame(tag: 0x12, body: bytes))
    }

    func resize(columns: Int, rows: Int) throws {
        guard !closed, decoder.geometryControl else { return }
        nextRequest += 1
        try enqueue(OwnerStream.frame(tag: 0x11, body: JSONEncoder().encode(
            ResizeRequest(id: nextRequest, kind: "resize", rows: rows, cols: columns))))
    }

    func close() {
        guard !closed else { return }
        closed = true
        connection.close()
        reader?.cancel()
        writer?.cancel()
        controlDeadline?.cancel()
        outgoing.finish()
        continuation.finish()
    }

    private func start() {
        reader = Task { [weak self] in
            guard let self else { return }
            do {
                while !closed {
                    for event in try decoder.receive(await connection.receiveBytes()) { try emit(event) }
                    if decoder.ready && !decoder.awaitingAttachment { controlDeadline?.cancel() }
                    if decoder.ended { close(); return }
                    await Task.yield()
                }
            } catch { fail("Session connection closed. Reconnect for a fresh screen; input was not retried.") }
        }
        writer = Task { [weak self] in
            guard let self else { return }
            do {
                for await bytes in writes {
                    guard !closed else { return }
                    try await connection.send(bytes)
                    pendingBytes -= bytes.count
                }
            } catch { fail("Input delivery is uncertain. Reconnect and inspect the prompt before resubmitting.") }
        }
    }

    private func emit(_ event: MobileTerminalEvent) throws {
        if case .control = event, decoder.awaitingAttachment { return }
        if case .dropped = continuation.yield(event) {
            throw RegistryQueryError("The phone fell behind Session output; reconnect for a fresh screen")
        }
    }

    private func enqueue(_ bytes: Data) throws {
        guard !closed, pendingBytes + bytes.count <= 65536 else {
            throw RegistryQueryError("The connection cannot keep up with input")
        }
        pendingBytes += bytes.count
        if case .dropped = outgoing.yield(bytes) {
            throw RegistryQueryError("The connection cannot keep up with input")
        }
    }

    private func fail(_ reason: String) {
        guard !closed else { return }
        continuation.yield(.disconnected(reason))
        close()
    }

    private func attachRequest(id: UInt64, control: Bool, size: TerminalGeometry) throws -> Data {
        try OwnerStream.frame(tag: 0x11, body: JSONEncoder().encode(AttachRequest(
            id: id, kind: "attach", control: control, replace: control, own: control ? "take" : nil,
            rows: size.rows, cols: size.columns, name: "Loopflow iPhone", moved: nil)))
    }
}

private struct AttachRequest: Encodable {
    let id: UInt64
    let kind: String
    let control: Bool
    let replace: Bool
    let own: String?
    let rows: Int
    let cols: Int
    let name: String?
    let moved: String?
}

private struct ResizeRequest: Encodable {
    let id: UInt64
    let kind: String
    let rows: Int
    let cols: Int
}
