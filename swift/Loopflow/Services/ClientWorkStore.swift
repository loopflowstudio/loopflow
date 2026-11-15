import Foundation

/// The immutable command and its original Machine, saved before admission.
/// The shape also reads the released creation journal without rewriting its words.
public struct SubmittedOperation: Codable, Sendable, Identifiable {
    public let requestID: String
    public let destination: String
    public let payload: Data
    public let version: Int
    public let submittedAt: Date
    public var id: String { requestID }

    func hasSameCommand(as other: SubmittedOperation) throws -> Bool {
        guard destination == other.destination else { return false }
        return try JSONDecoder().decode(JSONValue.self, from: payload) ==
            JSONDecoder().decode(JSONValue.self, from: other.payload)
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
    private let currentMachine: @Sendable () async throws -> String
    private let read: @Sendable (ServedRead) async throws -> String
    private let observe: @Sendable (String) -> ReceiptWatch
    private let send: @Sendable (Data) async throws -> Void
    private var publication: UInt64 = 0
    private var observers: [UUID: AsyncStream<Void>.Continuation] = [:]

    public init(directory: URL,
                prepareDirectory: @escaping @Sendable (URL) throws -> Void = { _ in },
                machine: @escaping @Sendable () async throws -> String,
                read: @escaping @Sendable (ServedRead) async throws -> String,
                observe: @escaping @Sendable (String) -> ReceiptWatch,
                send: @escaping @Sendable (Data) async throws -> Void) {
        self.directory = directory
        self.prepareDirectory = prepareDirectory
        currentMachine = machine
        self.read = read
        self.observe = observe
        self.send = send
    }

    private func disk() throws -> ClientWorkDatabase {
        if let database { return database }
        let opened = try ClientWorkDatabase(directory: directory, prepareDirectory: prepareDirectory)
        database = opened
        return opened
    }

    public func changes() -> AsyncStream<Void> {
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
        let requested: Set<String>
        let records: [SubmittedOperation]
        let queries: [String: ClientWorkQuery]
    }

    func presentation(machine: String, queries: [String: ServedRead]) throws -> Presentation {
        var readings: [String: ClientWorkQuery] = [:]
        for (key, read) in queries { readings[key] = try cachedPlans(machine: machine, for: read) }
        return Presentation(revision: publication, requested: Set(queries.keys), records: try pending().filter { $0.destination == machine }, queries: readings)
    }

    @discardableResult
    public func save(_ record: SubmittedOperation) throws -> UInt64 {
        try disk().save(record)
        changed()
        return publication
    }

    public func resolveMachine() async throws -> String { try await currentMachine() }

    public func isRecovering(_ id: String) -> Bool { active.contains(id) }

    public func pending() throws -> [SubmittedOperation] { try disk().pending() }

    public func receipt(_ id: String) throws -> OperationReceipt? { try disk().state(id)?.receipt }

    /// Current planning evidence is independent of the saved command result and
    /// of any Work query's coverage. A replaced database needs fresh evidence.
    public func planning(_ id: String) throws -> OperationPlanningReading? {
        guard let state = try disk().state(id) else { return nil }
        return try disk().currentPlanning(state)
    }

    /// Complete Work bases share the outbox transaction owner. A retired command
    /// leaves a reload requirement, so an old cached query cannot bring it back.
    public func receivePlans(_ plans: [WorkViewPlan], stamp: ReadingStamp, for read: ServedRead) throws -> ClientWorkQuery? {
        let result = try disk().cache(ClientWorkQuery(stamp: stamp, plans: plans), key: queryKey(read))
        changed()
        return result
    }

    public func cachedPlans(machine: String, for read: ServedRead) throws -> ClientWorkQuery? {
        try disk().cached(machine: machine, key: queryKey(read))
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
        let record = SubmittedOperation(requestID: requestID, destination: try await currentMachine(),
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
        guard try disk().state(record.id) != nil else { return }
        try await verify(record)
        let watch = observe(record.id)
        defer { watch.cancel() }
        var cursor = ReceiptCursor(machine: record.destination)
        for try await frame in watch.updates {
            try Task.checkCancellation()
            _ = try receive(frame, record: record, cursor: &cursor)
            if try disk().state(record.id) == nil { return }
        }
        try Task.checkCancellation()
        throw RegistryQueryError("Receipt observation ended; the original request is retained")
    }

    private func verify(_ record: SubmittedOperation) async throws {
        guard try await currentMachine() == record.destination else {
            throw RegistryQueryError("Waiting for the original Machine \(record.destination)")
        }
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
                    try await admit(record, epoch: epoch)
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
                try await admit(record, epoch: epoch)
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
