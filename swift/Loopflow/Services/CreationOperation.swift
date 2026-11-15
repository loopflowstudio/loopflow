import Foundation
import Observation

/// A submitted conversation has an operation identity before it has a Task or
/// Session. Native attachment and route selection belong to each host.
@MainActor @Observable
public final class CreationOperation: Identifiable {
    public let id: String
    public let request: String
    public let directory: String
    public let wave: String?
    public let graph: String?
    public let submittedAt: Date
    public private(set) var destination: String?
    public private(set) var saved = false
    public private(set) var receipt: TaskCreateReceipt?
    public private(set) var recovery: OperationRecovery?
    public private(set) var error: String?
    public private(set) var evidence: OperationReceipt?
    public private(set) var legacyOutcome: JSONValue?
    public var sessionID: String? {
        if let session = receipt?.sessionId { return session }
        if case .string(let id) = legacyOutcome?["receipt"]["session_id"] { return id }
        return nil
    }
    public var taskID: String? {
        if let receipt { return receipt.task.id }
        if case .string(let id) = legacyOutcome?["task_id"] { return id }
        return nil
    }

    init(id: String, request: String, directory: String, wave: String?, graph: String?, submittedAt: Date, destination: String?) {
        self.destination = destination
        self.id = id
        self.request = request
        self.directory = directory
        self.wave = wave
        self.graph = graph
        self.submittedAt = submittedAt
    }

    convenience init(_ value: CreationOperationSnapshot) {
        self.init(id: value.record.id, request: value.request, directory: value.directory,
            wave: value.wave, graph: value.graph, submittedAt: value.record.submittedAt, destination: value.record.destination)
        receive(value)
    }

    func markSaved() { saved = true }

    func receive(_ value: CreationOperationSnapshot) {
        saved = true
        destination = value.record.destination
        receipt = value.result
        evidence = value.receipt
        recovery = value.recovery
        legacyOutcome = value.legacyOutcome
        error = value.diagnostic ?? value.receipt?.error
    }

    func clearFailure() { error = nil }
    func fail(_ message: String) { error = message }
}

struct CreationOperationSnapshot: Sendable {
    let record: SubmittedOperation
    let request: String
    let directory: String
    let wave: String?
    let graph: String?
    let receipt: OperationReceipt?
    let result: TaskCreateReceipt?
    let recovery: OperationRecovery?
    let legacyOutcome: JSONValue?
    let diagnostic: String?

    init(record: SubmittedOperation, command: JSONValue, receipt: OperationReceipt?, recovery: OperationRecovery?, legacyOutcome: JSONValue?) throws {
        self.legacyOutcome = legacyOutcome
        self.record = record
        guard case .string(let directory) = command["directory"] else {
            throw RegistryQueryError("The saved creation has no repository; its original request is retained")
        }
        self.directory = directory
        if case .string(let text) = command["request"] { request = text }
        else if case .string(let title) = command["title"] { request = title }
        else { request = "" }
        if case .string(let name) = command["wave"] { wave = name } else { wave = nil }
        if case .string(let name) = command["graph"] { graph = name } else { graph = nil }
        self.receipt = receipt
        self.recovery = recovery
        let result = receipt?.result ?? receipt?.commits.last(where: { $0.stage == "task.filed" && $0.resolution == .committed })?.result
        // A candidate cannot replace the outcome of the original command.
        if receipt?.validation == .bound, let result, result != .null {
            do {
                let parsed = try JSONDecoder().decode(TaskCreateReceipt.self, from: JSONEncoder().encode(result))
                self.result = parsed
                diagnostic = nil
            } catch {
                self.result = nil
                diagnostic = "The saved creation result needs inspection. \(error.localizedDescription)"
            }
        } else {
            self.result = nil
            diagnostic = nil
        }
    }
}
