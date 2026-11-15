import Foundation
import Loopflow
import Observation

struct MobileCreationContext {
    let machineID: String
    let name: String
    let repositories: [PortfolioRepo]
}

@MainActor
protocol MobileTaskCreationClient {
    var interaction: WorkInteraction { get }
    func creationContext() async throws -> MobileCreationContext
    func importLegacyCreations(_ values: [LegacyOperation]) async throws
    func draftSubmitted(_ id: String, machine: String?) async throws -> Bool
}

/// Editor state only. Submitted commands and outcomes live in ClientWorkStore.
struct MobileTaskDraft: Codable, Equatable {
    var id: String? = UUID().uuidString.lowercased()
    var text = ""
    var directory = ""
    var machineID: String?
}

/// Decode released files without interpreting an old receipt as current evidence.
private struct LegacyTaskDraft: Decodable {
    struct Submission: Codable {
        let id: UUID
        let machineID: String
        let machineName: String
        let directory: String
        let request: String
    }
    struct Saved: Decodable {
        let submission: Submission
        let outcome: JSONValue?
    }
    let submission: Submission?
    let outcome: JSONValue?
    let savedRequests: [Saved]?

    func operations(submittedAt: Date) throws -> [LegacyOperation] {
        var requests = savedRequests ?? []
        if let submission { requests.append(Saved(submission: submission, outcome: outcome)) }
        return try requests.map { saved in
            let request = saved.submission
            if case .string(let id) = saved.outcome?["task_id"],
               case .string(let receiptID) = saved.outcome?["receipt"]["task"]["id"], id != receiptID {
                throw RegistryQueryError("The saved outcome names different Tasks; its original file is retained")
            }
            let id = request.id.uuidString.lowercased()
            let record = SubmittedOperation(requestID: id, destination: request.machineID,
                payload: try JSONEncoder().encode(ServedWrite.taskCreate(directory: request.directory,
                    requestID: id, title: nil, request: request.request, wave: nil, graph: nil, session: nil, run: true)),
                version: ServerProtocol.revision, submittedAt: submittedAt)
            // Keep the exact released outcome, including receipts from older schemas.
            return LegacyOperation(record: record, outcome: saved.outcome,
                source: try JSONDecoder().decode(JSONValue.self, from: JSONEncoder().encode(request)))
        }
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

    @MainActor
    func migrate(using client: any MobileTaskCreationClient) async throws {
        guard FileManager.default.fileExists(atPath: url.path) else { return }
        let source = try Data(contentsOf: url)
        let legacy = try JSONDecoder().decode(LegacyTaskDraft.self, from: source)
        let date = try url.resourceValues(forKeys: [.contentModificationDateKey]).contentModificationDate ?? Date()
        let operations = try legacy.operations(submittedAt: date)
        guard !operations.isEmpty else { return }
        try await client.importLegacyCreations(operations)
        // A crash before this rewrite repeats the idempotent import. A newer
        // editor saved while import awaited keeps its words and its whole file.
        guard try Data(contentsOf: url) == source else { return }
        var draft = try JSONDecoder().decode(MobileTaskDraft.self, from: source)
        if let submitted = legacy.submission, draft.text == submitted.request { draft.text = "" }
        try save(draft)
    }
}

@MainActor @Observable
final class MobileTaskCreation: Identifiable {
    private let client: any MobileTaskCreationClient
    private let store: MobileTaskDraftStore
    private(set) var draft = MobileTaskDraft()
    private var savedDraft = MobileTaskDraft()
    private(set) var context: MobileCreationContext?
    private(set) var busy = false
    private(set) var error: String?
    private(set) var storageUnavailable = false
    private var migrated = false
    private var submitted = false

    init(client: any MobileTaskCreationClient, store: MobileTaskDraftStore = .application, request: CreationOperation? = nil) {
        self.client = client
        self.store = store
        do { draft = try store.load(); savedDraft = draft }
        catch {
            storageUnavailable = true
            self.error = "Couldn’t read your saved draft. It hasn’t been overwritten."
        }
        if let request, !storageUnavailable {
            draft.text = request.request
            draft.id = UUID().uuidString.lowercased()
            draft.directory = request.directory
            draft.machineID = request.destination
        }
    }

    var canCreate: Bool {
        migrated && !submitted && !busy && !storageUnavailable && context != nil
            && !draft.text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty && !instructionsTooLong
            && context?.repositories.contains(where: { $0.path == draft.directory && $0.error == nil }) == true
    }
    var hasUnsavedChanges: Bool { draft != savedDraft }
    var instructionsTooLong: Bool {
        guard draft.text.utf8.count <= 32_768, let encoded = try? JSONEncoder().encode(draft.text) else { return true }
        return encoded.count > 48 * 1024
    }
    func editText(_ text: String) { if !storageUnavailable, draft.text != text { draft.text = text; draft.id = UUID().uuidString.lowercased() } }
    func selectRepository(_ path: String) { if !storageUnavailable, draft.directory != path { draft.directory = path; draft.id = UUID().uuidString.lowercased() } }

    @discardableResult
    func saveDraft() -> Bool {
        // No released submission fields may be removed before their import commits.
        guard migrated, !storageUnavailable, !submitted else { return false }
        do { try store.save(draft); savedDraft = draft; return true }
        catch { self.error = "Couldn’t save your changes on this phone. \(error.localizedDescription)"; return false }
    }

    func prepare() async {
        guard !storageUnavailable, !busy else { return }
        busy = true
        error = nil
        defer { busy = false }
        do {
            try await store.migrate(using: client)
            if !migrated {
                let saved = try store.load()
                if draft == savedDraft { draft = saved }
                savedDraft = saved
                migrated = true
            }
            client.interaction.start()
            if FileManager.default.fileExists(atPath: store.url.path), let id = draft.id,
               try await client.draftSubmitted(id, machine: draft.machineID), draft.id == id {
                draft.text = ""
                draft.id = UUID().uuidString.lowercased()
            }
            if draft.id == nil { draft.id = UUID().uuidString.lowercased() }
            let context = try await client.creationContext()
            try Task.checkCancellation()
            self.context = context
            if draft.machineID != context.machineID { draft.directory = "" }
            draft.machineID = context.machineID
            if draft.directory.isEmpty { draft.directory = context.repositories.first(where: { $0.error == nil })?.path ?? "" }
            _ = saveDraft()
        } catch { if !Task.isCancelled { self.error = error.localizedDescription } }
    }

    /// The caller installs the operation route and closes the editor in this call.
    func begin(select: (CreationOperation) -> Void) {
        guard canCreate else { return }
        let original = draft
        let operation = client.interaction.createTask(request: draft.text, directory: draft.directory, draftID: draft.id, select: select)
        submitted = true
        // The editor file remains intact until the outbox commits. Cleanup never
        // overwrites a newer draft and does not depend on the sheet's lifetime.
        let interaction = client.interaction
        let store = store
        Task {
            await interaction.persistPending()
            guard operation.saved, var current = try? store.load(), current == original else { return }
            current.text = ""
            current.id = UUID().uuidString.lowercased()
            try? store.save(current)
        }
    }
}
