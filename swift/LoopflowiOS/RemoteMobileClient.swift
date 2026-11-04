import Foundation
import Loopflow

@MainActor
final class RemoteMobileClient: MobileClient, MobileTaskCreationClient, MobileSessionOpeningClient, MobileSessionListingClient {
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

    func sessions(for task: WorkViewPlan.Content, after: String?) async throws -> SessionPage {
        try await verifyMachine(required: ["session_page", "repository_path"])
        let directory = try await read(.repositoryPath(id: task.work.repository))
            .trimmingCharacters(in: .whitespacesAndNewlines)
        let output = try await read(.sessionPage(directory: directory, headless: false, after: after, task: task.work.id))
        return try JSONDecoder().decode(SessionPage.self, from: Data(output.utf8))
    }

    func attach(sessionID: String) async throws -> any MobileAttachment {
        try await verifyMachine()
        let response = try await response(.sessionOwner(session: sessionID))
        guard case .sessionOwner(let owner) = response.result else { throw RegistryQueryError("The Machine did not resolve the Session owner") }
        guard let owner else { throw SessionNotRunning() }
        let connection = try MachineConnection(endpoint: credential.endpoint, token: credential.token)
        do {
            let response = try await connection.request(.attach(owner: owner))
            guard case .bridged = response.result else { throw RegistryQueryError("The Machine did not attach the Session") }
            return try await RemoteAttachment.open(sessionID: sessionID, connection: connection)
        } catch { connection.close(); throw error }
    }

    func startSession(sessionID: String) async throws {
        try await verifyMachine(required: ["session_start"])
        let reply = try await response(.command(.sessionStart(id: sessionID)))
        guard case .command(_, let output, let stderr, let code) = reply.result else {
            throw RegistryQueryError("The Machine did not confirm Session opening")
        }
        guard code == 0 else { throw RegistryQueryError(stderr.isEmpty ? "The Session did not open" : stderr) }
        // The complete CLI operation returns its existing SessionRecord.
        let record = try JSONDecoder().decode(SessionRecord.self, from: Data(output.utf8))
        guard record.id == sessionID else {
            throw RegistryQueryError("The Machine returned a different Session")
        }
    }

    func creationContext() async throws -> MobileCreationContext {
        try await verifyMachine(required: ["task_create", "portfolio", "view_plan"])
        let portfolio = try JSONDecoder().decode(Portfolio.self, from: Data(try await read(.portfolio).utf8))
        if let error = portfolio.error { throw RegistryQueryError(error) }
        guard let machineID = credential.machineID else { throw RegistryQueryError("Machine identity is unavailable") }
        return MobileCreationContext(machineID: machineID, name: credential.endpoint.host ?? "Paired Machine", repositories: portfolio.repos)
    }

    func createTask(_ submission: MobileTaskSubmission) async throws -> TaskCreateOutcome {
        try await verifySubmission(submission)
        let reply = try await response(.command(.taskCreate(directory: submission.directory,
            requestID: submission.id, request: submission.request, wave: nil, graph: nil, terminal: true)))
        guard case .command(_, let output, let stderr, let code) = reply.result else {
            throw RegistryQueryError("The Machine did not return the creation result")
        }
        guard code == 0 else { throw RegistryQueryError(stderr.isEmpty ? "Task creation did not finish" : stderr) }
        return try JSONDecoder().decode(TaskCreateOutcome.self, from: Data(output.utf8))
    }

    func createdTask(_ submission: MobileTaskSubmission, taskID: String) async throws -> WorkViewPlan.Content {
        try await verifySubmission(submission)
        let output = try await read(.viewPlan(directory: submission.directory, expression: "task:\(taskID)"))
        let tasks = try mobileTasks(JSONDecoder().decode(WorkViewPlan.self, from: Data(output.utf8)))
        guard let task = tasks.first(where: { $0.work.id == taskID }) else {
            throw RegistryQueryError("The Task was created, but its workspace is not available yet. Try Open Task again.")
        }
        return task
    }

    private func verifySubmission(_ submission: MobileTaskSubmission) async throws {
        try await verifyMachine(required: ["task_create", "view_plan"])
        guard submission.machineID == credential.machineID else {
            throw RegistryQueryError("Reconnect \(submission.machineName) to recover this request. Nothing was sent to this Machine.")
        }
    }

    private func verifyMachine(required: [String] = []) async throws {
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
        guard (["all_view_plans", "session_owner", "owner_bridge"] + required).allSatisfy(capabilities.contains) else {
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
    lazy var events = AsyncStream<MobileTerminalEvent>(unfolding: { [weak self] in
        await self?.nextEvent()
    }, onCancel: { [weak self] in
        Task { @MainActor in self?.close() }
    })
    private let connection: MachineConnection
    private var decoder = OwnerStream()
    private var pendingEvents: ArraySlice<MobileTerminalEvent> = []
    private var failure: String?
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
        (writes, outgoing) = AsyncStream.makeStream(bufferingPolicy: .bufferingOldest(64))
    }

    static func open(sessionID: String, connection: MachineConnection) async throws -> RemoteAttachment {
        let link = RemoteAttachment(sessionID: sessionID, connection: connection)
        let timeout = connection.deadline(seconds: 5)
        defer { timeout.cancel() }
        link.decoder.expectAttachment(1)
        try await connection.send(link.attachRequest(id: 1, control: false, size: TerminalGeometry(columns: 80, rows: 24)))
        while !link.decoder.ready && !link.decoder.ended {
            link.pendingEvents.append(contentsOf: try link.decoder.receive(await connection.receiveBytes()))
        }
        connection.keepAlive()
        link.startWriter()
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
        writer?.cancel()
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
