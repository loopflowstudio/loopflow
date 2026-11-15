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
    private var database: ClientWorkDatabase?
    private var active: Set<String> = []
    private let currentMachine: @Sendable () async throws -> String
    private let read: @Sendable (ServedRead) async throws -> String
    private let observe: @Sendable (String) -> ReceiptWatch
    private let send: @Sendable (Data) async throws -> Void
    private var observers: [UUID: AsyncStream<Void>.Continuation] = [:]

    public init(directory: URL,
                machine: @escaping @Sendable () async throws -> String,
                read: @escaping @Sendable (ServedRead) async throws -> String,
                observe: @escaping @Sendable (String) -> ReceiptWatch,
                send: @escaping @Sendable (Data) async throws -> Void) {
        self.directory = directory
        currentMachine = machine
        self.read = read
        self.observe = observe
        self.send = send
    }

    private func disk() throws -> ClientWorkDatabase {
        if let database { return database }
        let opened = try ClientWorkDatabase(directory: directory)
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
    private func changed() { for observer in observers.values { observer.yield(()) } }

    public func save(_ record: SubmittedOperation) throws {
        try disk().save(record)
        changed()
    }

    public func isRecovering(_ id: String) -> Bool { active.contains(id) }

    public func pending() throws -> [SubmittedOperation] { try disk().pending() }

    public func receipt(_ id: String) throws -> OperationReceipt? { try disk().state(id)?.receipt }

    public func presented(_ id: String) async throws {
        guard let state = try disk().state(id) else { return }
        guard try await currentMachine() == state.record.destination else {
            throw RegistryQueryError("Waiting for the original Machine \(state.record.destination)")
        }
        try Task.checkCancellation()
        guard let current = try disk().state(id) else { return }
        guard current.receipt?.delivery == .finished, current.receipt?.validation == .bound,
              current.receiptEpoch == current.observedEpoch else {
            throw RegistryQueryError("The operation is still unresolved; its request is retained")
        }
        try disk().remove(id)
        changed()
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

    private func recover(_ record: SubmittedOperation) async throws -> OperationReceipt? {
        guard try await currentMachine() == record.destination else {
            throw RegistryQueryError("Waiting for the original Machine \(record.destination)")
        }
        guard let saved = try disk().state(record.id), try saved.record.hasSameCommand(as: record) else {
            throw RegistryQueryError("The original saved operation is unavailable")
        }
        let watch = observe(record.id)
        defer { watch.cancel() }
        var cursor = ReceiptCursor(machine: record.destination)
        var submitted = false
        for try await frame in watch.updates {
            try Task.checkCancellation()
            if let reason = frame.unavailable { throw RegistryQueryError(reason) }
            guard let stamp = frame.stamp else {
                throw RegistryQueryError("The Machine returned an unstamped receipt reading")
            }
            let body = frame.body
            guard try cursor.receive(body, stamp: stamp) else { continue }
            try disk().observeEpoch(stamp.epoch, id: record.id)
            let state = try disk().state(record.id)
            guard let state else { return nil } // already presented by another window
            if let receipt = body {
                if state.receiptEpoch == stamp.epoch, let known = state.receipt, known.revision > receipt.revision { continue }
                guard receipt.requestID == record.id else {
                    throw RegistryQueryError("The Machine returned the receipt for another operation")
                }
                try disk().receive(receipt, epoch: stamp.epoch, id: record.id)
                changed()
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
                    try await admit(record, epoch: stamp.epoch)
                    submitted = true
                    watch.checkpoint(1)
                    continue
                }
                if receipt.delivery == .finished || receipt.delivery == .uncertain || receipt.error != nil || receipt.validation == .conflict {
                    return receipt
                }
                // After this call's admission, progress only observes the worker.
            } else if !submitted {
                guard state.sentEpoch == nil || state.sentEpoch == stamp.epoch else {
                    throw RegistryQueryError("The Machine's database changed; inspect the original request before continuing")
                }
                try await admit(record, epoch: stamp.epoch)
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
            if let revision, receipt.revision <= revision { return false }
            revision = receipt.revision
        } else if revision != nil { return false }
        return true
    }
}
