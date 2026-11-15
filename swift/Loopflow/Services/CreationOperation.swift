import Foundation
import Observation
import SwiftUI

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

    public enum Action: String, Identifiable, Sendable {
        case tryAgain = "Try again", edit = "Edit request", check = "Check request", inspect = "Inspect", open = "Retry opening"
        public var id: String { rawValue }
    }
    public struct Presentation {
        public let status: String
        public let actions: [Action]
        public let evidence: String?
    }
    private var needsInspection: Bool {
        if error != nil { return true }
        if evidence?.validation == .conflict { return true }
        // Normal preparation continues after its filed result is shown. Its
        // observer stays alive, and any later failure makes the request visible.
        if evidence?.delivery == .queued || evidence?.delivery == .dispatched {
            return recovery?.receiptAvailable == false
        }
        return recovery?.needsInspection == true
    }
    public var isListed: Bool { recovery?.resultPresented != true || needsInspection }

    /// Native readiness stays with the attachment owner; both hosts use the same
    /// recovery vocabulary for the submitted command.
    public func presentation(openingError: String? = nil) -> Presentation {
        var actions: [Action] = []
        if !saved, error != nil { actions += [.tryAgain, .edit] }
        if saved, receipt == nil || needsInspection { actions.append(.check) }
        if sessionID != nil, openingError != nil { actions.append(.open) }
        let evidence = evidence.flatMap { try? JSONEncoder().encode($0) }
            ?? legacyOutcome.flatMap { try? JSONEncoder().encode($0) }
        if evidence != nil, needsInspection { actions.append(.inspect) }
        var messages = [openingError, error].compactMap { $0 }
        if saved, needsInspection, self.evidence?.delivery == .finished || self.evidence?.delivery == .uncertain {
            messages += recovery?.messages ?? []
        }
        var seen = Set<String>()
        messages = messages.filter { seen.insert($0).inserted }
        return Presentation(status: messages.isEmpty ? "Starting conversation…" : messages.joined(separator: "\n"),
            actions: actions, evidence: evidence.map { String(decoding: $0, as: UTF8.self) })
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
        if evidence?.revision != value.receipt?.revision { error = nil }
        evidence = value.receipt
        recovery = value.recovery
        legacyOutcome = value.legacyOutcome
        error = value.diagnostic ?? value.receipt?.error ?? error
    }

    func clearFailure() { error = nil }
    func fail(_ message: String) { error = message }
}

/// Recovery controls stay available beside a live conversation as well as its
/// pending row. Hosts supply only their native opening and editor actions.
public struct CreationRequestActions: View {
    public let presentation: CreationOperation.Presentation
    public let perform: (CreationOperation.Action) -> Void
    @State private var inspecting = false

    public init(presentation: CreationOperation.Presentation, perform: @escaping (CreationOperation.Action) -> Void) {
        self.presentation = presentation
        self.perform = perform
    }

    public var body: some View {
        ForEach(presentation.actions) { action in
            Button(action.rawValue) {
                if action == .inspect { inspecting = true }
                else { perform(action) }
            }
        }
        .sheet(isPresented: $inspecting) {
            VStack(alignment: .leading, spacing: 12) {
                Text("Request receipt").font(.headline)
                ScrollView {
                    Text(presentation.evidence ?? "").font(.system(.caption, design: .monospaced))
                        .textSelection(.enabled).frame(maxWidth: .infinity, alignment: .leading)
                }
                Button("Done") { inspecting = false }
            }
            .padding(20)
            .frame(minWidth: 320, minHeight: 300)
            .task { perform(.inspect) }
        }
    }
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
