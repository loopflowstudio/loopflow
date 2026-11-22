import Foundation
import Loopflow

/// One control connection to the Machine while the app is in use, opened on
/// first use and reopened after a loss; terminals get connections of their own.
@MainActor
final class RemoteMobileClient: MobileTaskCreationClient {
    let savedMachine: SavedMachine
    /// Where its control connections open.
    private let endpoint: ServerEndpoint
    private var opening: Task<ServerLink, any Error>?

    /// How long a request may wait for its answer.
    static let requestDeadline = Duration.seconds(15)

    init(savedMachine: SavedMachine, endpoint: ServerEndpoint) {
        self.savedMachine = savedMachine
        self.endpoint = endpoint
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
    private lazy var journal: CreationSubmissions = {
        let directory = URL.applicationSupportDirectory.appending(path: "submissions-\(savedMachine.machineID)")
        let machineID = savedMachine.machineID
        return CreationSubmissions(directory: directory, machine: { machineID }, lookup: { [self] id in
            try await connection().read(.creationGet(requestID: id), deadline: Self.requestDeadline)
        }, send: { [self] payload in
            let creation = try JSONDecoder().decode(JSONValue.self, from: payload)
            return String(decoding: try await connection().request(.submittedCreation(creation), deadline: Self.requestDeadline).bytes,
                          as: UTF8.self)
        })
    }()

    /// The Machine's typed reads and writes, as Desktop makes them.
    var query: RegistryQuery {
        RegistryQuery.served(deadline: Self.requestDeadline) { [self] in try await connection() }
    }

    /// Every repository's Tasks matching `expression`, kept current.
    func observeTasks(expression: String, saved: SavedReadings, subscription: String) -> RemoteTaskObservation {
        RemoteTaskObservation(query: .viewSelect(expression: expression), saved: saved, subscription: subscription) { [self] in
            try await connection()
        }
    }

    /// One Task as `lf task show` reads it, kept current: each update is a whole reading as sent.
    func observeTask(issue: String) -> ServerWatch<RawJSON> {
        // The Machine finds a Task by its ID from any directory, as Desktop asks.
        ServerLink.subscribe(.taskGet(directory: "/", issue: issue), connect: { [self] in try await connection() }, map: { update in
            guard case .reading(let frame) = update else { return nil }
            return frame
        })
    }

    /// The Machine's Waves that are still in use.
    func waves() async throws -> [Wave] {
        try await query.allWaves().filter { $0.status != .abandoned }
    }

    private struct Ticket: Decodable { let ticket: String }

    /// Attaching fits the terminal to `size`.
    func attach(sessionID: String, size: TerminalGeometry) async throws -> RemoteAttachment {
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

    /// Saved before it is sent, then sent once; the answer names the Task and
    /// its conversation, so opening it reads nothing more.
    func startTask(_ submission: MobileTaskSubmission) async throws -> MobileTaskStart {
        try verifySubmission(submission)
        let id = submission.id.uuidString.lowercased()
        let answer = try await journal.submit(taskCreate(submission, requestID: id), requestID: id)
        do { return MobileTaskStart(try RegistryQuery.decode(TaskCreated.self, from: answer)) }
        catch { throw SubmittedCreationError(requestID: id, cause: error) }
    }

    /// A saved request asks the Machine first and resends the same payload
    /// only when the Machine says to continue.
    func recoverTask(_ submission: MobileTaskSubmission) async throws -> MobileTaskOutcome {
        try verifySubmission(submission)
        let id = submission.id.uuidString.lowercased()
        let record = try await journal.record(taskCreate(submission, requestID: id), requestID: id)
        guard let lookup = try await journal.reconcile(record) else {
            throw RegistryQueryError("This request is already being sent")
        }
        guard lookup.kind == "task.create", lookup.run, let taskID = lookup.taskId else {
            throw RegistryQueryError("The Machine returned a different creation receipt")
        }
        let receipt = try lookup.result.map { try JSONDecoder().decode(TaskCreateReceipt.self, from: JSONEncoder().encode($0)) }
        guard receipt == nil || receipt?.task.id == taskID else {
            throw RegistryQueryError("The creation receipt names a different Task")
        }
        return MobileTaskOutcome(taskId: taskID,
            state: lookup.disposition == .complete ? .started : .unknown,
            receipt: receipt, message: lookup.disposition == .inspect ? lookup.explanation : nil)
    }

    private func taskCreate(_ submission: MobileTaskSubmission, requestID: String) -> ServedWrite {
        .taskCreate(directory: submission.directory, requestID: requestID, title: nil,
                    request: submission.request, wave: nil, graph: nil, session: nil, run: true)
    }

    /// The created Task is on screen: its saved creation is done.
    func taskPresented(_ submission: MobileTaskSubmission) async throws {
        try await journal.presented(submission.id.uuidString.lowercased())
    }

    func createdTask(_ submission: MobileTaskSubmission, taskID: String) async throws -> MobileTask {
        try verifySubmission(submission)
        guard let task = try await task(id: taskID, directory: submission.directory) else {
            throw RegistryQueryError("The Task was created, but its workspace is not available yet. Try Open Task again.")
        }
        return task
    }

    /// One Task's row as a reading of its repository lists it, with the
    /// conversation it has now; nil when that reading does not list it.
    func task(id: String, directory: String) async throws -> MobileTask? {
        let plan = try await query.viewPlan(expression: "task:\(id)", repoPath: directory)
        return try mobileTasks(plan).first { $0.work.id == id }.map(MobileTask.init)
    }

    private func verifySubmission(_ submission: MobileTaskSubmission) throws {
        guard submission.machineID == savedMachine.machineID else {
            throw RegistryQueryError("Reconnect \(submission.machineName) to recover this request. Nothing was sent to this Machine.")
        }
    }

    /// The Machine's identity from its welcome. The negotiated revision is the
    /// compatibility check; a server that speaks it offers every operation here.
    private static func identity(_ welcome: ServerWelcome) throws -> String {
        guard case .object(let fields) = welcome.machine, case .string(let machineID) = fields["id"], !machineID.isEmpty else {
            throw RegistryQueryError("The Machine did not return its identity")
        }
        return machineID
    }

    /// The live control connection, opened once for concurrent callers.
    private func connection() async throws -> ServerLink {
        let link: ServerLink
        if let opening, let live = try? await opening.value, await !live.isClosed {
            link = live
        } else {
            let endpoint = endpoint
            let task = Task { try await ServerLink.open(endpoint) }
            opening = task
            do { link = try await task.value } catch {
                if opening == task { opening = nil }
                throw error
            }
            if opening != task { await link.close(); return try await connection() }
        }
        guard try Self.identity(link.welcome) == savedMachine.machineID else {
            throw RegistryQueryError("This address now identifies a different Machine. Forget this connection and scan its code again.")
        }
        return link
    }

    /// A Task change under its request ID; a repeat after a lost answer runs nothing.
    func change(_ change: ServedWrite, requestID: String) async throws -> Revisions? {
        try await connection().write(change, requestID: requestID).revisions
    }

    private func request(_ operation: ServerOperation) async throws -> RawJSON {
        try await connection().request(operation, deadline: Self.requestDeadline)
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
    private var attachDeadline: Task<Void, Never>?

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

    /// Take the size back after another device took it.
    func takeSize(_ size: TerminalGeometry) throws {
        guard !closed, decoder.ready else { throw RegistryQueryError("The Session is not ready") }
        nextRequest += 1
        decoder.expectAttachment(nextRequest)
        attachDeadline = connection.deadline(seconds: 5)
        try enqueue(attachRequest(id: nextRequest, size: size))
    }

    func send(_ bytes: Data) throws {
        guard !closed, decoder.ready else { throw RegistryQueryError("The Session is not ready") }
        try enqueue(OwnerStream.input(bytes))
    }

    func resize(columns: Int, rows: Int) throws {
        guard !closed, decoder.sizing else { return }
        nextRequest += 1
        try enqueue(OwnerStream.resizeRequest(id: nextRequest, columns: columns, rows: rows))
    }

    func close() {
        guard !closed else { return }
        closed = true
        connection.close()
        writer?.cancel()
        heartbeat?.cancel()
        attachDeadline?.cancel()
        pendingEvents = []
        outgoing.finish()
    }

    /// The renderer pulls the next event. Keep only one decoded network batch;
    /// do not turn packet coalescing into an artificial slow-consumer failure.
    private func nextEvent() async -> TerminalEvent? {
        while !closed && !Task.isCancelled {
            if let event = pendingEvents.popFirst() {
                if pendingEvents.isEmpty { pendingEvents = [] }
                if case .sizing = event, decoder.awaitingAttachment { continue }
                return event
            }
            if decoder.ended { close(); return nil }
            do {
                let bytes = try await connection.receiveBytes()
                guard !closed, !Task.isCancelled else { break }
                pendingEvents = ArraySlice(try decoder.receive(bytes))
                if decoder.ready && !decoder.awaitingAttachment { attachDeadline?.cancel() }
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
