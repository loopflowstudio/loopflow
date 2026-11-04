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

struct MobileSavedTaskRequest: Codable, Identifiable {
    let submission: MobileTaskSubmission
    var outcome: TaskCreateOutcome?
    var id: UUID { submission.id }
}

struct MobileTaskDraft: Codable {
    var text = ""
    var directory = ""
    var machineID: String?
    var submission: MobileTaskSubmission?
    var outcome: TaskCreateOutcome?
    var savedRequests: [MobileSavedTaskRequest] = []

    init() {}

    private enum CodingKeys: String, CodingKey {
        case text, directory, machineID, submission, outcome, savedRequests
    }

    init(from decoder: any Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        text = try values.decode(String.self, forKey: .text)
        directory = try values.decode(String.self, forKey: .directory)
        machineID = try values.decodeIfPresent(String.self, forKey: .machineID)
        submission = try values.decodeIfPresent(MobileTaskSubmission.self, forKey: .submission)
        outcome = try values.decodeIfPresent(TaskCreateOutcome.self, forKey: .outcome)
        // Preserve drafts saved before the local recovery list was introduced.
        // This file is phone-owned UI state, not a Machine wire DTO.
        savedRequests = try values.decodeIfPresent([MobileSavedTaskRequest].self, forKey: .savedRequests) ?? []
    }
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
        persist(draft)
    }

    private func persist(_ next: MobileTaskDraft) -> Bool {
        guard !storageUnavailable else { return false }
        do { try store.save(next); draft = next; return true }
        catch { self.error = "Couldn’t save your changes on this phone. \(error.localizedDescription)"; return false }
    }

    @discardableResult
    func saveForLater() -> Bool {
        guard !busy, let submission = draft.submission else { return false }
        var next = draft
        next.savedRequests.insert(MobileSavedTaskRequest(submission: submission, outcome: draft.outcome), at: 0)
        next.submission = nil
        next.outcome = nil
        next.text = ""
        if let context, next.machineID != context.machineID {
            next.machineID = context.machineID
            next.directory = context.repositories.first(where: { $0.error == nil })?.path ?? ""
        }
        guard persist(next) else { return false }
        error = nil
        return true
    }

    func savedRequest(_ id: UUID) -> MobileSavedTaskRequest? {
        draft.savedRequests.first { $0.id == id }
    }

    func clearError() {
        if !storageUnavailable { error = nil }
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
    func submit(savedRequestID: UUID? = nil) async {
        guard !busy, !storageUnavailable else { return }
        let saved = savedRequestID.flatMap { savedRequest($0) }
        if savedRequestID != nil && saved == nil { return }
        let previous = savedRequestID == nil ? draft.outcome : saved?.outcome
        if let previous, previous.state != .inProgress { return }
        if savedRequestID == nil && draft.submission == nil {
            guard canCreate, let context else { return }
            draft.submission = MobileTaskSubmission(id: UUID(), machineID: context.machineID,
                machineName: context.name, directory: draft.directory, request: draft.text)
        }
        // Even a failed local write leaves the same ID in memory for the next attempt.
        guard saveDraft(), let submission = saved?.submission ?? draft.submission else { return }
        busy = true
        error = nil
        defer { busy = false }
        do {
            let outcome = try await client.createTask(submission)
            try Task.checkCancellation()
            if let previous, previous.taskId != outcome.taskId {
                throw RegistryQueryError("The Machine returned a different Task for this request")
            }
            if let receipt = outcome.receipt, receipt.task.id != outcome.taskId {
                throw RegistryQueryError("The Machine returned a receipt for a different Task")
            }
            var next = draft
            if let savedRequestID {
                guard let index = next.savedRequests.firstIndex(where: { $0.id == savedRequestID }) else { return }
                next.savedRequests[index].outcome = outcome
            } else {
                next.outcome = outcome
            }
            _ = persist(next)
        } catch {
            if !Task.isCancelled {
                self.error = "Couldn’t confirm the result. Check status to recover this same request. \(error.localizedDescription)"
            }
        }
    }

    func openTask(savedRequestID: UUID? = nil) async -> WorkViewPlan.Content? {
        guard !busy, !storageUnavailable else { return nil }
        let saved = savedRequestID.flatMap { savedRequest($0) }
        if savedRequestID != nil && saved == nil { return nil }
        guard let submission = saved?.submission ?? draft.submission,
              let outcome = savedRequestID == nil ? draft.outcome : saved?.outcome else { return nil }
        busy = true
        error = nil
        defer { busy = false }
        do {
            let task = try await client.createdTask(submission, taskID: outcome.taskId)
            try Task.checkCancellation()
            guard task.work.id == outcome.taskId else { throw RegistryQueryError("The Machine returned a different Task") }
            // Clear recovery only after a successful read; the list then opens this recorded Session.
            var next = draft
            if let savedRequestID {
                next.savedRequests.removeAll { $0.id == savedRequestID }
            } else {
                next.text = ""
                next.submission = nil
                next.outcome = nil
                next.directory = submission.directory
                next.machineID = submission.machineID
            }
            guard persist(next) else { return nil }
            return task
        } catch {
            if !Task.isCancelled { self.error = error.localizedDescription }
            return nil
        }
    }
}
