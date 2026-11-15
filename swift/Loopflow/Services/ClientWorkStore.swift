import Foundation

/// The immutable command and its original Machine, saved before admission.
/// The shape also reads the released creation journal without rewriting its words.
public struct SubmittedOperation: Codable, Sendable, Identifiable {
    public let requestID: String
    public let destination: String
    public let payload: Data
    public let version: Int
    public let submittedAt: Date
    public let draftID: String?
    public init(requestID: String, destination: String, payload: Data, version: Int, submittedAt: Date, draftID: String? = nil) {
        self.requestID = requestID
        self.destination = destination
        self.payload = payload
        self.version = version
        self.submittedAt = submittedAt
        self.draftID = draftID
    }
    public var id: String { requestID }

    func hasSameCommand(as other: SubmittedOperation) throws -> Bool {
        guard destination == other.destination else { return false }
        return try JSONDecoder().decode(JSONValue.self, from: payload) ==
            JSONDecoder().decode(JSONValue.self, from: other.payload)
    }
}

/// Released client evidence has no send epoch. Import never makes it unsent.
public struct LegacyOperation: Sendable {
    public let record: SubmittedOperation
    public let outcome: JSONValue?
    public let source: JSONValue?
    public init(record: SubmittedOperation, outcome: JSONValue?, source: JSONValue? = nil) {
        self.record = record
        self.outcome = outcome
        self.source = source
    }
}

public struct OperationSubmissionError: LocalizedError {
    public let requestID: String
    public let cause: any Error
    public init(requestID: String, cause: any Error) {
        self.requestID = requestID
        self.cause = cause
    }
    public var errorDescription: String? {
        "Request \(requestID) is saved for recovery. \(cause.localizedDescription)"
    }
}

/// One serialized disk and receipt owner for both native clients. Admission
/// replies acknowledge saving only; stamped subscriptions establish outcomes.
public actor ClientWorkStore {
    public typealias ReceiptWatch = ServerWatch<SubscriptionReading<OperationReceipt>>
    private let directory: URL
    private let prepareDirectory: @Sendable (URL) throws -> Void
    private var database: ClientWorkDatabase?
    private var active: Set<String> = []
    private nonisolated let connection: MachineConnection?
    private var connectionChanges: Task<Void, Never>?
    private let currentMachine: @Sendable () async throws -> String
    private let read: @Sendable (ServedRead) async throws -> String
    private let observe: @Sendable (String) -> ReceiptWatch
    private let send: @Sendable (Data) async throws -> Void
    private var publication: UInt64 = 0
    private var observers: [UUID: AsyncStream<Void>.Continuation] = [:]

    public init(directory: URL, connection: MachineConnection? = nil,
                prepareDirectory: @escaping @Sendable (URL) throws -> Void = { _ in },
                machine: @escaping @Sendable () async throws -> String,
                read: @escaping @Sendable (ServedRead) async throws -> String,
                observe: @escaping @Sendable (String) -> ReceiptWatch,
                send: @escaping @Sendable (Data) async throws -> Void) {
        self.connection = connection
        self.directory = directory
        self.prepareDirectory = prepareDirectory
        currentMachine = machine
        self.read = read
        self.observe = observe
        self.send = send
    }

    deinit { connectionChanges?.cancel() }

    /// Release local readers and SQLite after consumers have stopped. Saved
    /// commands remain on disk; a later access can reopen the same store.
    public func close() async {
        let changes = connectionChanges
        connectionChanges = nil
        changes?.cancel()
        for observer in observers.values { observer.finish() }
        observers = [:]
        await changes?.value
        database = nil
    }

    nonisolated func rememberMachine(_ machine: String) { connection?.rememberMachine(machine) }

    private func disk() throws -> ClientWorkDatabase {
        if let database { return database }
        let opened = try ClientWorkDatabase(directory: directory, prepareDirectory: prepareDirectory)
        database = opened
        return opened
    }

    public func changes() -> AsyncStream<Void> {
        if connectionChanges == nil, let connection {
            connectionChanges = Task { [weak self] in
                for await _ in connection.changes() {
                    guard !Task.isCancelled else { return }
                    await self?.changed()
                }
            }
        }
        let id = UUID()
        let (stream, continuation) = AsyncStream<Void>.makeStream(bufferingPolicy: .bufferingNewest(1))
        observers[id] = continuation
        continuation.yield(())
        continuation.onTermination = { [weak self] _ in Task { await self?.removeObserver(id) } }
        return stream
    }

    private func removeObserver(_ id: UUID) { observers.removeValue(forKey: id) }
    private func changed() {
        publication &+= 1
        for observer in observers.values { observer.yield(()) }
    }

    struct Presentation: Sendable {
        let revision: UInt64
        let waitingForConnection: Bool
        let names: [String: SessionNameFact]
        let requested: Set<String>
        let records: [SubmittedOperation]
        let queries: [String: ClientWorkQuery]
        let operations: [PendingOperationSnapshot]
    }

    func retainQueries(machine: String, keys: Set<String>) throws {
        try disk().retainQueries(machine: machine, keys: keys)
    }

    func presentation(machine: String, queries: [String: ServedRead], reusable: Set<String>? = nil) throws -> Presentation {
        try retainQueries(machine: machine, keys: reusable ?? Set(queries.keys))
        var readings: [String: ClientWorkQuery] = [:]
        for (key, read) in queries { readings[key] = try cachedPlans(machine: machine, for: read) }
        let records = try pending().filter { $0.destination == machine }
        let waiting = connection?.isWaiting(for: machine) == true
        let operations = try records.map { record in
            let command = try JSONDecoder().decode(JSONValue.self, from: record.payload)
            var snapshot = try PendingOperationSnapshot(record: record, command: command,
                receipt: receipt(record.id), recovery: recovery(record.id), legacyOutcome: disk().state(record.id)?.legacyOutcome)
            snapshot.waitingForConnection = waiting
            return snapshot
        }
        return Presentation(revision: publication, waitingForConnection: waiting, names: try disk().names(machine: machine), requested: Set(queries.keys), records: records, queries: readings, operations: operations)
    }

    public func receiveNames(_ records: [SessionRecord], stamp: ReadingStamp) throws {
        try disk().receiveNames(records, stamp: stamp)
        changed()
    }

    @discardableResult
    public func save(_ record: SubmittedOperation) throws -> UInt64 {
        try disk().save(record)
        changed()
        return publication
    }

    public func importLegacy(_ values: [LegacyOperation]) throws {
        try disk().importLegacy(values)
        changed()
    }

    /// Import an unavailable Machine's local evidence without creating a
    /// transport or changing its destination. The same database owns the commit.
    public static func importLegacy(_ values: [LegacyOperation], directory: URL,
                                    prepareDirectory: @escaping @Sendable (URL) throws -> Void) async throws {
        try await Task.detached {
            try ClientWorkDatabase(directory: directory, prepareDirectory: prepareDirectory).importLegacy(values)
        }.value
    }

    public static func draftSubmitted(_ id: String, directory: URL,
                                      prepareDirectory: @escaping @Sendable (URL) throws -> Void) async throws -> Bool {
        try await Task.detached {
            try ClientWorkDatabase(directory: directory, prepareDirectory: prepareDirectory).draftSubmitted(id)
        }.value
    }

    public func draftSubmitted(_ id: String) throws -> Bool { try disk().draftSubmitted(id) }

    public func resolveMachine() async throws -> String {
        let machine = try await currentMachine()
        rememberMachine(machine)
        return machine
    }

    public func isRecovering(_ id: String) -> Bool { active.contains(id) }

    public func pending() throws -> [SubmittedOperation] { try disk().pending() }

    public func receipt(_ id: String) throws -> OperationReceipt? { try disk().state(id)?.receipt }

    /// Current planning evidence is independent of the saved command result and
    /// of any Work query's coverage. A replaced database needs fresh evidence.
    public func planning(_ id: String) throws -> OperationPlanningReading? {
        guard let state = try disk().state(id) else { return nil }
        return try disk().currentPlanning(state)
    }

    /// Independently supported fields join before exact-query replacement. A
    /// partial answer can update a parent without certifying filtered absence.
    public func receivePlans(_ plans: [WorkViewPlan], stamp: ReadingStamp, for read: ServedRead) throws -> ClientWorkQuery? {
        // These callers carry wire stamps but no receiving-link attribution.
        // A buffered pre-reconnect reading cannot borrow the current link's
        // generation to revalidate external facts. SQL-only fields still join.
        let result = try disk().cache(ClientWorkQuery(stamp: stamp, plans: plans), key: queryKey(read))
        changed()
        return result
    }

    public func cachedPlans(machine: String, for read: ServedRead) throws -> ClientWorkQuery? {
        try disk().cached(machine: machine, key: queryKey(read))
    }

    func fieldReading(machine: String, work: WorkWorkspaceKey) throws -> WorkFieldReading? {
        try disk().fieldReading(machine: machine, work: work, observation: connection?.observationConnection)
    }

    public func forgetQueries(machine: String) throws {
        try disk().forgetQueries(machine: machine)
        changed()
    }

    private func queryKey(_ read: ServedRead) throws -> String {
        let encoder = JSONEncoder()
        encoder.outputFormatting = .sortedKeys
        return String(decoding: try encoder.encode(read), as: UTF8.self)
    }

    /// Acknowledge showing a saved result. Retirement independently checks
    /// domain resolution and persists the remaining query reload obligations.
    public func presented(_ id: String) throws {
        try disk().presented(id)
        changed()
    }

    public func recovery(_ id: String) throws -> OperationRecovery? {
        guard let state = try disk().state(id) else { return nil }
        let epoch = try disk().activeEpoch(machine: state.record.destination)
        return OperationRecovery(receipt: state.receipt, planning: try planning(id),
            resultPresented: state.presentedAt != nil,
            receiptAvailable: state.receiptUnavailable != true && epoch != nil && state.receiptEpoch == epoch)
    }

    public func record(_ command: ServedWrite, requestID: String) async throws -> SubmittedOperation {
        let record = SubmittedOperation(requestID: requestID, destination: try await resolveMachine(),
            payload: try JSONEncoder().encode(command), version: ServerProtocol.revision, submittedAt: Date())
        try save(record)
        // Return the original record, including its timestamp and sequence.
        guard let saved = try disk().state(requestID) else {
            throw RegistryQueryError("The saved operation disappeared")
        }
        return saved.record
    }

    public func submit(_ command: ServedWrite, requestID: String) async throws -> String {
        guard active.insert(requestID).inserted else {
            throw OperationSubmissionError(requestID: requestID, cause: RegistryQueryError("This request is already being recovered"))
        }
        defer { active.remove(requestID); changed() }
        let record = try await record(command, requestID: requestID)
        do {
            guard let receipt = try await recover(record) else {
                throw RegistryQueryError("This request is already being recovered")
            }
            guard receipt.delivery == .finished, receipt.validation == .bound, let result = receipt.result else {
                throw RegistryQueryError(receipt.error ?? "The operation's outcome is unresolved")
            }
            return String(decoding: try JSONEncoder().encode(result), as: UTF8.self)
        } catch { throw OperationSubmissionError(requestID: requestID, cause: error) }
    }

    /// Subscribe before sending. A missing answer authorizes only an unsent
    /// request, or an idempotent admission retry in the same database lifetime.
    /// A changed lifetime never authorizes replay of a possibly executed command.
    public func reconcile(_ record: SubmittedOperation) async throws -> OperationReceipt? {
        guard active.insert(record.id).inserted else { return nil }
        defer { active.remove(record.id); changed() }
        return try await recover(record)
    }

    /// Keep current domain evidence after recovering the saved result. This
    /// observation never submits a command, including after a missing receipt.
    /// The caller owns its lifetime independently of navigation or attachment.
    public func observeSaved(_ record: SubmittedOperation) async throws {
        guard active.insert(record.id).inserted else { return }
        defer { active.remove(record.id); changed() }
        guard let state = try disk().state(record.id), state.retiredAt == nil else { return }
        try await verify(record)
        let watch = observe(record.id)
        defer { watch.cancel() }
        var cursor = ReceiptCursor(machine: record.destination)
        for try await frame in watch.updates {
            try Task.checkCancellation()
            _ = try receive(frame, record: record, cursor: &cursor)
            if try disk().state(record.id)?.retiredAt != nil || disk().state(record.id) == nil { return }
        }
        try Task.checkCancellation()
        throw RegistryQueryError("Receipt observation ended; the original request is retained")
    }

    private func verify(_ record: SubmittedOperation) async throws {
        guard let saved = try disk().state(record.id), try saved.record.hasSameCommand(as: record) else {
            throw RegistryQueryError("The original saved operation is unavailable")
        }
    }

    /// Both one-shot recovery and continued observation reduce through the same
    /// persisted ordering. An older first frame after restart returns the known
    /// receipt instead of presenting its historical error as current recovery.
    private func receive(_ frame: SubscriptionReading<OperationReceipt>, record: SubmittedOperation,
                         cursor: inout ReceiptCursor) throws -> ClientWorkDatabase.State? {
        if let reason = frame.unavailable {
            try disk().unavailable(record.id)
            changed()
            throw RegistryQueryError(reason)
        }
        guard let stamp = frame.stamp else {
            throw RegistryQueryError("The Machine returned an unstamped receipt reading")
        }
        if let receipt = frame.body, receipt.requestID != record.id {
            throw RegistryQueryError("The Machine returned the receipt for another operation")
        }
        guard try cursor.receive(frame.body, stamp: stamp) else { return nil }
        guard try disk().observeEpoch(stamp.epoch, id: record.id) else { return try disk().state(record.id) }
        if let receipt = frame.body {
            try disk().receive(receipt, epoch: stamp.epoch, id: record.id)
        } else {
            try disk().unavailable(record.id)
        }
        let state = try disk().state(record.id)
        try disk().retireIfConverged(record.id)
        changed()
        return state
    }

    private func recover(_ record: SubmittedOperation) async throws -> OperationReceipt? {
        try await verify(record)
        for await _ in changes() {
            try Task.checkCancellation()
            if try !disk().earlierRenamePending(record) { break }
        }
        let watch = observe(record.id)
        defer { watch.cancel() }
        var cursor = ReceiptCursor(machine: record.destination)
        var submitted = false
        for try await frame in watch.updates {
            try Task.checkCancellation()
            guard let state = try receive(frame, record: record, cursor: &cursor) else {
                if try disk().state(record.id) == nil { return nil }
                continue
            }
            // receive validated the stamp before changing the durable state.
            guard let epoch = try disk().activeEpoch(machine: record.destination) else { continue }
            if state.receiptEpoch == epoch, let receipt = state.receipt {
                let canContinue: Bool
                if !submitted && (receipt.delivery == .dispatched || receipt.delivery == .uncertain),
                   receipt.kind == "task.create" || receipt.kind == "session.new" {
                    let domain = try JSONDecoder().decode(CreationLookup?.self,
                        from: Data(try await read(.creationGet(requestID: record.id)).utf8))
                    guard domain == nil || domain?.requestId == record.id else {
                        throw RegistryQueryError("The Machine returned recovery for another creation")
                    }
                    canContinue = domain?.disposition == .continue || domain?.disposition == .complete
                } else { canContinue = false }
                if !submitted && (canContinue || receipt.delivery == .untracked || receipt.delivery == .queued || receipt.validation == .unbound) {
                    do { try await admit(record, epoch: epoch) }
                    catch is ServerConnectionUnavailable { continue }
                    submitted = true
                    watch.checkpoint(1)
                    continue
                }
                if receipt.delivery == .finished || receipt.delivery == .uncertain || receipt.error != nil || receipt.validation == .conflict {
                    return receipt
                }
                // After this call's admission, progress only observes the worker.
            } else if !submitted {
                guard state.sentEpoch == nil || state.sentEpoch == epoch else {
                    throw RegistryQueryError("The Machine's database changed; inspect the original request before continuing")
                }
                do { try await admit(record, epoch: epoch) }
                catch is ServerConnectionUnavailable { continue }
                submitted = true
                watch.checkpoint(1)
            }
        }
        throw RegistryQueryError("Receipt observation ended; the original request is retained")
    }

    private func admit(_ record: SubmittedOperation, epoch: String) async throws {
        guard record.version == 5 || record.version == ServerProtocol.revision else {
            throw RegistryQueryError("This saved request requires protocol revision \(record.version)")
        }
        try disk().markSent(record.id, epoch: epoch)
        let command = try JSONDecoder().decode(JSONValue.self, from: record.payload)
        let envelope: JSONValue = .object(["kind": .string("operation.submit"),
            "request_id": .string(record.id), "epoch": .string(epoch), "command": command])
        try await send(JSONEncoder().encode(envelope))
    }
}

/// Subscription order establishes epoch changes. Revisions order replies only
/// within that epoch; once departed, an old epoch cannot replace the new base.
struct ReceiptCursor {
    let machine: String
    private var epoch: String?
    private var departed: Set<String> = []
    private var revision: Int64?

    init(machine: String) { self.machine = machine }

    mutating func receive(_ receipt: OperationReceipt?, stamp: ReadingStamp) throws -> Bool {
        guard stamp.machine == machine, stamp.schema == ServerProtocol.revision, stamp.complete else {
            throw RegistryQueryError("The receipt does not cover this Machine and protocol")
        }
        if stamp.epoch != epoch {
            guard !departed.contains(stamp.epoch) else { return false }
            if let epoch { departed.insert(epoch) }
            epoch = stamp.epoch
            revision = nil
        }
        if let receipt {
            if let revision, receipt.revision < revision { return false }
            revision = receipt.revision
        }
        return true
    }
}
