import Foundation
import Loopflow
import Observation

struct MobileCreationContext {
    let machineID: String
    let name: String
    let repositories: [PortfolioRepo]
}

/// Local recovery state, not another Machine wire type. Credentials stay in Keychain.
struct MobileTaskSubmission: Codable, Equatable {
    let id: UUID
    let machineID: String
    let machineName: String
    let directory: String
    let request: String
}

@MainActor
protocol MobileTaskCreationClient {
    func creationContext() async throws -> MobileCreationContext
    func createTask(_ submission: MobileTaskSubmission) async throws -> TaskCreateOutcome
    func createdTask(_ submission: MobileTaskSubmission, taskID: String) async throws -> WorkViewPlan.Content
}

struct MobileTaskDraft: Codable {
    var text = ""
    var directory = ""
    var machineID: String?
    var submission: MobileTaskSubmission?
    var outcome: TaskCreateOutcome?
}

struct MobileTaskDraftStore {
    let url: URL

    static var application: Self {
        Self(url: URL.applicationSupportDirectory.appending(path: "Loopflow/new-task.json"))
    }

    func load() throws -> MobileTaskDraft {
        guard FileManager.default.fileExists(atPath: url.path) else { return MobileTaskDraft() }
        return try JSONDecoder().decode(MobileTaskDraft.self, from: Data(contentsOf: url))
    }

    func save(_ draft: MobileTaskDraft) throws {
        try FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
        try JSONEncoder().encode(draft).write(to: url, options: [.atomic, .completeFileProtection])
        var excluded = url
        var values = URLResourceValues()
        values.isExcludedFromBackup = true
        try excluded.setResourceValues(values)
    }
}

@MainActor
@Observable
final class MobileTaskCreation {
    private let client: any MobileTaskCreationClient
    private let store: MobileTaskDraftStore
    private(set) var draft = MobileTaskDraft()
    private(set) var context: MobileCreationContext?
    private(set) var busy = false
    private(set) var error: String?
    private(set) var storageUnavailable = false

    init(client: any MobileTaskCreationClient, store: MobileTaskDraftStore = .application) {
        self.client = client
        self.store = store
        do { draft = try store.load() }
        catch {
            storageUnavailable = true
            self.error = "Couldn’t read your saved draft. Close this sheet and try again. It hasn’t been overwritten."
        }
    }

    var canCreate: Bool {
        !busy && !storageUnavailable && draft.submission == nil && context != nil
            && !draft.text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
            && !instructionsTooLong
            && context?.repositories.contains(where: { $0.path == draft.directory && $0.error == nil }) == true
    }

    var settled: Bool { draft.outcome.map { $0.state != .inProgress } ?? false }

    var instructionsTooLong: Bool {
        guard draft.text.utf8.count <= 32_768,
              let encoded = try? JSONEncoder().encode(draft.text) else { return true }
        // Escaped characters also count toward the bounded Machine frame.
        return encoded.count > 48 * 1024
    }

    func editText(_ text: String) {
        guard draft.submission == nil, !storageUnavailable else { return }
        draft.text = text
    }

    func selectRepository(_ path: String) {
        guard draft.submission == nil, !storageUnavailable else { return }
        draft.directory = path
    }

    @discardableResult
    func saveDraft() -> Bool {
        guard !storageUnavailable else { return false }
        do { try store.save(draft); return true }
        catch { self.error = "Couldn’t save this draft. Nothing new was sent. \(error.localizedDescription)"; return false }
    }

    func prepare() async {
        guard !storageUnavailable, !busy else { return }
        busy = true
        error = nil
        defer { busy = false }
        do {
            let context = try await client.creationContext()
            try Task.checkCancellation()
            self.context = context
            if let submission = draft.submission {
                guard submission.machineID == context.machineID else {
                    throw RegistryQueryError("This request belongs to \(submission.machineName). Reconnect that Machine to check or open it.")
                }
            } else {
                if draft.machineID != context.machineID { draft.directory = "" }
                draft.machineID = context.machineID
                if draft.directory.isEmpty { draft.directory = context.repositories.first(where: { $0.error == nil })?.path ?? "" }
                _ = saveDraft()
            }
        } catch { if !Task.isCancelled { self.error = error.localizedDescription } }
    }

    /// Reusing this method after uncertain delivery always submits the saved intent.
    func submit() async {
        guard !busy, !storageUnavailable, !settled else { return }
        if draft.submission == nil {
            guard canCreate, let context else { return }
            draft.submission = MobileTaskSubmission(id: UUID(), machineID: context.machineID,
                machineName: context.name, directory: draft.directory, request: draft.text)
        }
        // Even a failed local write leaves the same ID in memory for the next attempt.
        guard saveDraft(), let submission = draft.submission else { return }
        busy = true
        error = nil
        defer { busy = false }
        do {
            let outcome = try await client.createTask(submission)
            try Task.checkCancellation()
            if let previous = draft.outcome, previous.taskId != outcome.taskId {
                throw RegistryQueryError("The Machine returned a different Task for this request")
            }
            if let receipt = outcome.receipt, receipt.task.id != outcome.taskId {
                throw RegistryQueryError("The Machine returned a receipt for a different Task")
            }
            draft.outcome = outcome
            _ = saveDraft()
        } catch {
            if !Task.isCancelled {
                self.error = "Couldn’t confirm the result. Check status to recover this same request. \(error.localizedDescription)"
            }
        }
    }

    func openTask() async -> WorkViewPlan.Content? {
        guard !busy, let submission = draft.submission, let outcome = draft.outcome else { return nil }
        busy = true
        error = nil
        defer { busy = false }
        do {
            let task = try await client.createdTask(submission, taskID: outcome.taskId)
            try Task.checkCancellation()
            guard task.work.id == outcome.taskId else { throw RegistryQueryError("The Machine returned a different Task") }
            // Clear recovery only after a successful read; the list then opens this recorded Session.
            var next = MobileTaskDraft()
            next.directory = submission.directory
            next.machineID = submission.machineID
            try store.save(next)
            draft = next
            return task
        } catch { self.error = error.localizedDescription; return nil }
    }
}
