import Foundation
import Loopflow

/// One control connection to the Machine while the app is in use, opened on
/// first use and reopened after a loss; terminals get connections of their own.
@MainActor
final class RemoteMobileClient: MobileTaskCreationClient {
    let savedMachine: SavedMachine
    private var opening: Task<MachineConnection, any Error>?
    private var live: MachineConnection?

    init(savedMachine: SavedMachine) { self.savedMachine = savedMachine }

    static func connect(endpoint: URL, replacing: SavedMachine? = nil) async throws -> SavedMachine {
        let connection = try await MachineConnection.open(endpoint: endpoint)
        defer { connection.close() }
        let machineID = try identity(connection.welcome)
        if let replacing, replacing.machineID != machineID {
            throw RegistryQueryError("This address belongs to a different Machine. Scan the code from your original Machine’s Mobile settings.")
        }
        try Task.checkCancellation()
        let saved = SavedMachine(endpoint: endpoint, machineID: machineID)
        try MachineKeychain.save(saved)
        return saved
    }


    /// Every repository's Tasks matching `expression`, kept current.
    func observeTasks(expression: String) async throws -> RemoteTaskObservation {
        try await RemoteTaskObservation.open(connection: connection(), query: .viewSelect(expression: expression))
    }

    func repositoryPath(id: String) async throws -> String {
        try await decode(String.self, read(.repoPath(id: id)))
    }


    /// The Machine's Waves that are still in use.
    func waves() async throws -> [MobileWave] {
        try await decode([MobileWave].self, read(.waveList)).filter { $0.retiredAt == nil && $0.status != "abandoned" }
    }

    func file(task: String, path: String) async throws -> TaskFileSnapshot {
        try await decode(TaskFileSnapshot.self, read(.taskFile(issue: task, path: path)))
    }

    func changes(task: String) async throws -> TaskChangesSnapshot {
        try await decode(TaskChangesSnapshot.self, read(.taskChanges(issue: task, base: "parent", head: nil)))
    }

    func diff(task: String, path: String) async throws -> TaskDiffSnapshot {
        try await decode(TaskDiffSnapshot.self, read(.taskDiff(issue: task, path: path, base: "parent", head: nil, draft: nil)))
    }

    func suspend() {
        opening?.cancel()
        opening = nil
        live?.close()
        live = nil
    }

    /// Attaching takes the keyboard and fits the terminal to `size`.
    func attach(sessionID: String, size: TerminalGeometry) async throws -> RemoteAttachment {
        let connection = try await connection()
        let terminal: MachineSocket
        do { terminal = try await connection.attach(session: sessionID) }
        catch let error as ServerError where error.code == .notFound { throw SessionNotRunning() }
        do { return try await RemoteAttachment.open(sessionID: sessionID, socket: terminal, size: size) }
        catch { terminal.close(); throw error }
    }

    /// Continue a recorded conversation whose terminal has stopped.
    func startSession(sessionID: String) async throws {
        let record = try decode(SessionRecord.self, await request(.command(.sessionStart(id: sessionID))))
        guard record.id == sessionID else {
            throw RegistryQueryError("The Machine returned a different Session")
        }
    }

    func creationContext() async throws -> MobileCreationContext {
        let portfolio = try await decode(Portfolio.self, read(.desktopPortfolio))
        if let error = portfolio.error { throw RegistryQueryError(error) }
        let machineID = savedMachine.machineID
        return MobileCreationContext(machineID: machineID, name: savedMachine.endpoint.host ?? "Machine", repositories: portfolio.repos)
    }

    func createTask(_ submission: MobileTaskSubmission) async throws -> MobileTaskOutcome {
        try verifySubmission(submission)
        let id = submission.id.uuidString.lowercased()
        let previous = try await decode(CreationLookup?.self, read(.creationGet(requestID: id)))
        if let previous {
            guard previous.requestId == id, previous.kind == "task.create", previous.run,
                  let taskID = previous.taskId else {
                throw RegistryQueryError("The Machine returned a different creation receipt")
            }
            let receipt = try previous.result.map { try decode(TaskCreateReceipt.self, $0) }
            guard receipt == nil || receipt?.task.id == taskID else {
                throw RegistryQueryError("The creation receipt names a different Task")
            }
            if previous.disposition != .continue {
                return MobileTaskOutcome(taskId: taskID,
                    state: previous.disposition == .complete ? .started : .unknown,
                    receipt: receipt, message: previous.disposition == .inspect ? previous.explanation : nil)
            }
        }
        let result = try await request(.command(.taskCreate(directory: submission.directory,
            requestID: id, title: nil, request: submission.request, wave: nil, graph: nil, session: nil, run: true)))
        let receipt = try decode(TaskCreateReceipt.self, result)
        return MobileTaskOutcome(taskId: receipt.task.id, state: .started, receipt: receipt, message: nil)
    }

    func createdTask(_ submission: MobileTaskSubmission, taskID: String) async throws -> WorkViewPlan.Content {
        try verifySubmission(submission)
        let plan = try await decode(WorkViewPlan.self, read(.desktopViewGet(directory: submission.directory, expression: "task:\(taskID)")))
        guard let task = try mobileTasks(plan).first(where: { $0.work.id == taskID }) else {
            throw RegistryQueryError("The Task was created, but its workspace is not available yet. Try Open Task again.")
        }
        return task
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
    private func connection() async throws -> MachineConnection {
        let connection: MachineConnection
        if let opening, let live = try? await opening.value, !live.isClosed {
            connection = live
        } else {
            let endpoint = savedMachine.endpoint
            let task = Task { try await MachineConnection.open(endpoint: endpoint) }
            opening = task
            do { connection = try await task.value } catch {
                if opening == task { opening = nil }
                throw error
            }
            if opening == task { live = connection } else { connection.close(); return try await self.connection() }
        }
        guard try Self.identity(connection.welcome) == savedMachine.machineID else {
            throw RegistryQueryError("This address now identifies a different Machine. Forget this connection and scan its code again.")
        }
        return connection
    }

    private func request(_ operation: ServerOperation) async throws -> JSONValue {
        try await connection().request(operation)
    }

    private func read(_ query: ServedRead) async throws -> JSONValue {
        try await request(.read(query))
    }

    private func decode<T: Decodable>(_ type: T.Type, _ value: JSONValue) throws -> T {
        try JSONDecoder().decode(type, from: JSONEncoder().encode(value))
    }
}

/// A Wave as the nav offers it: a name within one repository.
struct MobileWave: Decodable, Identifiable, Equatable {
    let id: String
    let name: String
    let status: String
    /// The repository's path on the Machine.
    let repo: String
    let retiredAt: String?

    enum CodingKeys: String, CodingKey {
        case id, name, status, repo
        case retiredAt = "retired_at"
    }
}

@MainActor
final class RemoteAttachment {
    let sessionID: String
    lazy var events = AsyncStream<MobileTerminalEvent>(unfolding: { [weak self] in
        await self?.nextEvent()
    }, onCancel: { [weak self] in
        Task { @MainActor in self?.close() }
    })
    private let connection: MachineSocket
    private var decoder = OwnerStream()
    private var pendingEvents: ArraySlice<MobileTerminalEvent> = []
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
        try await connection.send(link.attachRequest(id: 1, size: size))
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
        writer?.cancel()
        heartbeat?.cancel()
        controlDeadline?.cancel()
        pendingEvents = []
        outgoing.finish()
    }

    /// The renderer pulls the next event. Keep only one decoded network batch;
    /// do not turn packet coalescing into an artificial slow-consumer failure.
    private func nextEvent() async -> MobileTerminalEvent? {
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
        return failure.map(MobileTerminalEvent.disconnected)
    }

    private func startWriter() {
        writer = Task { [weak self] in
            guard let self else { return }
            do {
                for await bytes in writes {
                    guard !closed else { return }
                    try await connection.send(bytes)
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
        try OwnerStream.frame(tag: 0x11, body: JSONEncoder().encode(AttachRequest(
            id: id, kind: "attach", control: true, replace: true, own: "take",
            rows: size.rows, cols: size.columns, name: "Loopflow iPhone", moved: nil)))
    }
}

private struct AttachRequest: Encodable {
    let id: UInt64
    let kind: String
    let control: Bool
    let replace: Bool
    let own: String
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
