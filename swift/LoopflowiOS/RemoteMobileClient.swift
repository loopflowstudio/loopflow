import Foundation
import Loopflow

/// One control connection to the Machine while the app is in use, opened on
/// first use and reopened after a loss; terminals get connections of their own.
@MainActor
final class RemoteMobileClient: MobileTaskCreationClient {
    let savedMachine: SavedMachine
    nonisolated let connection: MachineConnection
    #if DEBUG
    var attachForTesting: ((String, TerminalGeometry) async throws -> RemoteAttachment)?
    #endif

    /// How long a request may wait for its answer.
    static let requestDeadline = Duration.seconds(15)

    init(savedMachine: SavedMachine) {
        self.savedMachine = savedMachine
        connection = MachineConnection(machine: savedMachine.machineID) {
            let link = try await ServerLink.open(.machine(savedMachine.endpoint))
            do {
                guard try Self.identity(link.welcome) == savedMachine.machineID else {
                    throw RegistryQueryError("This address now identifies a different Machine. Forget this connection and scan its code again.")
                }
                return link
            } catch { await link.close(); throw error }
        }
    }

    static func connect(endpoint: URL, replacing: SavedMachine? = nil) async throws -> SavedMachine {
        let link = try await ServerLink.open(.machine(endpoint))
        let machineID: String
        do { machineID = try identity(link.welcome) } catch { await link.close(); throw error }
        await link.close()
        if let replacing, replacing.machineID != machineID {
            throw RegistryQueryError("This address belongs to a different Machine. Scan the code from your original Machine’s Mobile settings.")
        }
        try Task.checkCancellation()
        let saved = SavedMachine(endpoint: endpoint, machineID: machineID)
        try MachineKeychain.save(saved)
        return saved
    }


    /// Creations saved before their first send, for this Machine.
    lazy var journal: ClientWorkStore = {
        let directory = URL.applicationSupportDirectory.appending(path: "submissions-\(savedMachine.machineID)")
        let machineID = savedMachine.machineID
        let connection = connection
        return ClientWorkStore(directory: directory, connection: connection, prepareDirectory: { directory in
            try FileManager.default.setAttributes([.protectionKey: FileProtectionType.completeUntilFirstUserAuthentication],
                ofItemAtPath: directory.path)
        }, machine: { machineID }, read: { query in
            return try await connection.link().request(.read(query), deadline: Self.requestDeadline).printed
        }, observe: { id in
            return ServerLink.subscribe(.operationGet(requestID: id), reconnect: true, connection: connection) { update in
                guard case .reading(let frame) = update else { return nil }
                return try frame.decode(SubscriptionReading<OperationReceipt>.self)
            }
        }, send: { payload in
            let creation = try JSONDecoder().decode(JSONValue.self, from: payload)
            let link = try await connection.link()
            do { _ = try await link.request(.submittedOperation(creation), deadline: Self.requestDeadline) }
            catch {
                if await link.isClosed { throw ServerConnectionUnavailable(error) }
                throw error
            }
        })
    }()

    lazy var interaction = WorkInteraction(machine: savedMachine.machineID, store: journal)

    /// The Machine's typed reads and writes, as Desktop makes them.
    var query: RegistryQuery {
        RegistryQuery.served(deadline: Self.requestDeadline) { [self] in try await connection.link() }
    }

    /// Every repository's Tasks matching `expression`, kept current.
    func observeTasks(expression: String) -> RemoteTaskObservation {
        RemoteTaskObservation(query: .viewSelect(expression: expression), interaction: interaction, connection: connection)
    }

    /// The Machine's Waves that are still in use.
    func waves() async throws -> [Wave] {
        try await query.allWaves().filter { $0.status != .abandoned }
    }

    private struct Ticket: Decodable { let ticket: String }

    /// Attaching takes the keyboard and fits the terminal to `size`.
    func attach(sessionID: String, size: TerminalGeometry) async throws -> RemoteAttachment {
        #if DEBUG
        if let attachForTesting { return try await attachForTesting(sessionID, size) }
        #endif
        let ticket: String
        do { ticket = try decode(Ticket.self, await request(.sessionAttach(session: sessionID))).ticket }
        catch let error as ServerError where error.code == .notFound { throw SessionNotRunning() }
        let terminal = try await MachineSocket.terminal(endpoint: savedMachine.endpoint, ticket: ticket)
        do { return try await RemoteAttachment.open(sessionID: sessionID, socket: terminal, size: size) }
        catch { terminal.close(); throw error }
    }

    private struct Started: Decodable {
        let terminalID: String
        enum CodingKeys: String, CodingKey { case terminalID = "terminal_id" }
    }

    /// Continue a recorded conversation whose terminal has stopped, in a
    /// terminal the Machine holds; repeating `request` starts nothing.
    func startSession(sessionID: String, request: String, size: TerminalGeometry) async throws {
        _ = try decode(Started.self, await self.request(.sessionContinue(
            session: sessionID, request: request, rows: size.rows, cols: size.columns)))
    }

    func creationContext() async throws -> MobileCreationContext {
        let portfolio = try await query.portfolio()
        if let error = portfolio.error { throw RegistryQueryError(error) }
        let machineID = savedMachine.machineID
        return MobileCreationContext(machineID: machineID, name: savedMachine.endpoint.host ?? "Machine", repositories: portfolio.repos)
    }

    func draftSubmitted(_ id: String, machine: String?) async throws -> Bool {
        guard let machine, machine != savedMachine.machineID else { return try await journal.draftSubmitted(id) }
        return try await ClientWorkStore.draftSubmitted(id,
            directory: URL.applicationSupportDirectory.appending(path: "submissions-\(machine)"),
            prepareDirectory: { directory in
                try FileManager.default.setAttributes([.protectionKey: FileProtectionType.completeUntilFirstUserAuthentication],
                    ofItemAtPath: directory.path)
            })
    }


    func importLegacyCreations(_ values: [LegacyOperation]) async throws {
        for (machine, records) in Dictionary(grouping: values, by: { $0.record.destination }) {
            if machine == savedMachine.machineID { try await journal.importLegacy(records) }
            else {
                // Import to the original Machine's owner without opening or sending
                // through the currently connected Machine.
                let directory = URL.applicationSupportDirectory.appending(path: "submissions-\(machine)")
                try await ClientWorkStore.importLegacy(records, directory: directory, prepareDirectory: { directory in
                    try FileManager.default.setAttributes([.protectionKey: FileProtectionType.completeUntilFirstUserAuthentication],
                        ofItemAtPath: directory.path)
                })
            }
        }
    }

    /// The Machine's identity from its welcome. The negotiated revision is the
    /// compatibility check; a server that speaks it offers every operation here.
    private nonisolated static func identity(_ welcome: ServerWelcome) throws -> String {
        guard case .object(let fields) = welcome.machine, case .string(let machineID) = fields["id"], !machineID.isEmpty else {
            throw RegistryQueryError("The Machine did not return its identity")
        }
        return machineID
    }

    private func request(_ operation: ServerOperation) async throws -> RawJSON {
        try await connection.link().request(operation, deadline: Self.requestDeadline)
    }

    private func read(_ query: ServedRead) async throws -> RawJSON {
        try await request(.read(query))
    }

    private func decode<T: Decodable>(_ type: T.Type, _ value: RawJSON) throws -> T {
        try value.decode(type)
    }
}

@MainActor
final class RemoteAttachment {
    let sessionID: String
    lazy var events = AsyncStream<TerminalEvent>(unfolding: { [weak self] in
        await self?.nextEvent()
    }, onCancel: { [weak self] in
        Task { @MainActor in self?.close() }
    })
    private let connection: MachineSocket
    private var decoder = OwnerStream()
    private var pendingEvents: ArraySlice<TerminalEvent> = []
    private var failure: String?
    private var writer: Task<Void, Never>?
    private var heartbeat: Task<Void, Never>?
    private let writes: AsyncStream<Data>
    private let outgoing: AsyncStream<Data>.Continuation
    private var pendingBytes = 0
    private var nextRequest: UInt64 = 1
    private var closed = false
    private var controlDeadline: Task<Void, Never>?

    private init(sessionID: String, socket: MachineSocket) {
        self.sessionID = sessionID
        connection = socket
        (writes, outgoing) = AsyncStream.makeStream(bufferingPolicy: .bufferingOldest(64))
    }

    /// `socket` is an admitted terminal connection carrying the owner protocol.
    static func open(sessionID: String, socket connection: MachineSocket, size: TerminalGeometry) async throws -> RemoteAttachment {
        let link = RemoteAttachment(sessionID: sessionID, socket: connection)
        let timeout = connection.deadline(seconds: 5)
        defer { timeout.cancel() }
        link.decoder.expectAttachment(1)
        try await connection.sendBytes(link.attachRequest(id: 1, size: size))
        while !link.decoder.ready && !link.decoder.ended {
            link.pendingEvents.append(contentsOf: try link.decoder.receive(await connection.receiveBytes()))
        }
        connection.keepAlive()
        link.startWriter()
        link.startHeartbeat()
        return link
    }

    /// Take the keyboard and size back after another device took them.
    func takeControl(size: TerminalGeometry) throws {
        guard !closed, decoder.ready else { throw RegistryQueryError("The Session is not ready") }
        nextRequest += 1
        decoder.expectAttachment(nextRequest)
        controlDeadline = connection.deadline(seconds: 5)
        try enqueue(attachRequest(id: nextRequest, size: size))
    }

    func send(_ bytes: Data) throws {
        guard !closed, decoder.ready, decoder.inputControl else { throw RegistryQueryError("Keyboard control was released") }
        try enqueue(OwnerStream.input(bytes))
    }

    func resize(columns: Int, rows: Int) throws {
        guard !closed, decoder.geometryControl else { return }
        nextRequest += 1
        try enqueue(OwnerStream.resizeRequest(id: nextRequest, columns: columns, rows: rows))
    }

    func close() {
        guard !closed else { return }
        closed = true
        connection.close()
        writer?.cancel()
        heartbeat?.cancel()
        controlDeadline?.cancel()
        pendingEvents = []
        outgoing.finish()
    }

    /// The renderer pulls the next event. Keep only one decoded network batch;
    /// do not turn packet coalescing into an artificial slow-consumer failure.
    private func nextEvent() async -> TerminalEvent? {
        while !closed && !Task.isCancelled {
            if let event = pendingEvents.popFirst() {
                if pendingEvents.isEmpty { pendingEvents = [] }
                if case .control = event, decoder.awaitingAttachment { continue }
                return event
            }
            if decoder.ended { close(); return nil }
            do {
                let bytes = try await connection.receiveBytes()
                guard !closed, !Task.isCancelled else { break }
                pendingEvents = ArraySlice(try decoder.receive(bytes))
                if decoder.ready && !decoder.awaitingAttachment { controlDeadline?.cancel() }
                await Task.yield()
            } catch { fail("Session connection closed. Reconnect for a fresh screen; input was not retried.") }
        }
        guard !Task.isCancelled else { close(); return nil }
        defer { failure = nil }
        return failure.map(TerminalEvent.disconnected)
    }

    private func startWriter() {
        writer = Task { [weak self] in
            guard let self else { return }
            do {
                for await bytes in writes {
                    guard !closed else { return }
                    try await connection.sendBytes(bytes)
                    guard !closed else { return }
                    pendingBytes -= bytes.count
                }
            } catch { fail("Input delivery is uncertain. Reconnect and inspect the prompt before resubmitting.") }
        }
    }

    private func startHeartbeat() {
        heartbeat = Task { [weak self] in
            while !Task.isCancelled {
                do { try await Task.sleep(for: .seconds(10)) } catch { return }
                guard let self, !closed else { return }
                do {
                    nextRequest += 1
                    try enqueue(decoder.heartbeatRequest(id: nextRequest))
                } catch {
                    fail("Session connection closed. Reconnect for a fresh screen; input was not retried.")
                    return
                }
            }
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
        failure = reason
        close()
    }

    private func attachRequest(id: UInt64, size: TerminalGeometry) throws -> Data {
        try OwnerStream.attachRequest(id: id, columns: size.columns, rows: size.rows, name: "Loopflow iPhone")
    }
}
