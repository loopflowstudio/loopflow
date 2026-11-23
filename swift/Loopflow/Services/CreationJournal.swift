import Foundation

/// A creation saved before its first send: the exact payload, and the Machine it is for.
public struct SubmittedCreation: Codable, Sendable, Identifiable {
    public let requestID: String
    public let destination: String
    public let payload: Data
    public let version: Int
    public let submittedAt: Date
    // Corrections replace the same journal file atomically under a fresh request ID.
    var journalID: String?
    var fileWorkflow: String?
    var refusal: ServerError?
    public var id: String { requestID }
    fileprivate var slot: String { journalID ?? id }

    public var refusedDraft: RefusedTaskDraft? {
        guard let refusal, let task = try? JSONDecoder().decode(SavedTask.self, from: payload),
              task.kind == "task.create", task.session == nil else { return nil }
        return RefusedTaskDraft(id: id, directory: task.directory, request: task.request,
            wave: task.wave, graph: task.graph ?? fileWorkflow, run: task.run, refusal: refusal)
    }
}

private struct SavedTask: Decodable {
    let kind: String
    let directory: String
    let request: String
    let wave: CreationWave?
    let graph: String?
    let session: String?
    let run: Bool
}

public struct RefusedTaskDraft: Identifiable, Equatable, LocalizedError {
    public let id: String
    public let directory: String
    public let request: String
    public let wave: CreationWave?
    public let graph: String?
    public let run: Bool
    public let refusal: ServerError
    public var errorDescription: String? { refusal.localizedDescription }
}

public struct SubmittedCreationError: LocalizedError {
    public let requestID: String
    public let cause: any Error
    public init(requestID: String, cause: any Error) {
        self.requestID = requestID
        self.cause = cause
    }
    public var errorDescription: String? {
        "Creation \(requestID) is saved for recovery. \(cause.localizedDescription)"
    }
}

/// Save before send, reconcile by request ID: a creation whose answer was lost
/// is asked about, then resent exactly, and retired once its result is on screen.
public actor CreationSubmissions {
    private let directory: URL
    private var active: Set<String> = []
    private let currentMachine: @Sendable () async throws -> String
    private let lookup: @Sendable (String) async throws -> ReadOutput
    private let send: @Sendable (Data) async throws -> String

    /// `lookup` prints `creation.get` for a request ID; `send` makes a submitted creation.
    public init(directory: URL,
                machine: @escaping @Sendable () async throws -> String,
                lookup: @escaping @Sendable (String) async throws -> ReadOutput,
                send: @escaping @Sendable (Data) async throws -> String) {
        self.directory = directory
        self.currentMachine = machine
        self.lookup = lookup
        self.send = send
    }

    private func url(_ id: String) -> URL {
        let name = Data(id.utf8).base64EncodedString().replacingOccurrences(of: "/", with: "_")
        return directory.appendingPathComponent(name).appendingPathExtension("json")
    }

    private func read(_ file: URL) throws -> SubmittedCreation {
        try JSONDecoder().decode(SubmittedCreation.self, from: Data(contentsOf: file))
    }

    private func write(_ record: SubmittedCreation) throws {
        try JSONEncoder().encode(record).write(to: url(record.slot), options: .atomic)
    }

    @discardableResult
    public func save(_ record: SubmittedCreation) throws -> SubmittedCreation {
        if let existing = try saved(record.id) {
            guard existing.destination == record.destination, existing.payload == record.payload else {
                throw RegistryQueryError("This creation identity already has another submitted request")
            }
            return existing
        }
        guard !FileManager.default.fileExists(atPath: url(record.slot).path) else {
            throw RegistryQueryError("This draft has already been replaced.")
        }
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        try write(record)
        return record
    }

    private func files() throws -> [URL] {
        guard FileManager.default.fileExists(atPath: directory.path) else { return [] }
        return try FileManager.default.contentsOfDirectory(at: directory, includingPropertiesForKeys: nil)
            .filter { $0.pathExtension == "json" }
    }

    public func pending() throws -> [SubmittedCreation] {
        try files().map(read).sorted { $0.submittedAt < $1.submittedAt }
    }

    private func saved(_ id: String) throws -> SubmittedCreation? {
        let file = url(id)
        if FileManager.default.fileExists(atPath: file.path) {
            let record = try read(file)
            return record.id == id ? record : nil
        }
        // A correction keeps the original filename. A damaged unrelated draft
        // still reports its error in pending() without preventing a new save.
        return try files().lazy.compactMap { try? self.read($0) }.first { $0.id == id }
    }

    public func presented(_ id: String) async throws {
        guard let record = try saved(id), record.refusal == nil else { return }
        guard try await currentMachine() == record.destination else {
            throw RegistryQueryError("Waiting for the original Machine \(record.destination)")
        }
        try Task.checkCancellation()
        if let current = try saved(id), current.refusal == nil {
            try FileManager.default.removeItem(at: url(record.slot))
        }
    }

    public func discardDraft(_ id: String) throws {
        guard let record = try saved(id), record.refusal != nil else { return }
        try FileManager.default.removeItem(at: url(record.slot))
    }

    /// Saves `change` under `requestID` for its Machine; saving it again is a no-op.
    public func record(_ change: ServedWrite, requestID: String) async throws -> SubmittedCreation {
        return try await record(change, requestID: requestID, fileWorkflow: nil, replacing: nil)
    }

    private func record(_ change: ServedWrite, requestID: String, fileWorkflow: String?,
                        replacing: String?) async throws -> SubmittedCreation {
        var record = SubmittedCreation(requestID: requestID, destination: try await currentMachine(),
            payload: try JSONEncoder().encode(change), version: ServerProtocol.revision, submittedAt: Date())
        record.fileWorkflow = fileWorkflow
        guard let replacing else { return try save(record) }
        guard let old = try saved(replacing), let draft = old.refusedDraft else {
            throw RegistryQueryError("This draft has already been discarded or submitted.")
        }
        guard record.destination == old.destination, change.cwd == draft.directory else {
            throw RegistryQueryError("Reopen this draft on its original Machine and repository.")
        }
        guard requestID != replacing, try saved(requestID) == nil else {
            throw RegistryQueryError("The correction needs a fresh request ID.")
        }
        record.journalID = old.slot
        try write(record)
        return record
    }

    public func submit(_ change: ServedWrite, requestID: String, fileWorkflow: String? = nil,
                       replacing: String? = nil) async throws -> String {
        guard active.insert(requestID).inserted else {
            throw SubmittedCreationError(requestID: requestID, cause: RegistryQueryError("This creation is already being sent"))
        }
        defer { active.remove(requestID) }
        let record = try await record(change, requestID: requestID, fileWorkflow: fileWorkflow, replacing: replacing)
        if let draft = record.refusedDraft { throw draft }
        do { return try await send(record.payload) }
        catch {
            if case .taskCreate(_, _, _, _, _, _, nil, _) = change, let refusal = Self.refusal(error),
               let confirmed = try? await confirmRefusal(refusal, for: record, keepDraft: replacing != nil) {
                throw confirmed
            }
            throw SubmittedCreationError(requestID: requestID, cause: error)
        }
    }

    private func confirmRefusal(_ refusal: ServerError, for record: SubmittedCreation,
                                keepDraft: Bool) async throws -> (any Error)? {
        guard try await receipt(for: record) == nil else { return nil }
        if keepDraft, let draft = try park(record, refusal: refusal) { return draft }
        try FileManager.default.removeItem(at: url(record.slot))
        return refusal
    }

    private static func refusal(_ error: Error) -> ServerError? {
        guard let refusal = error as? ServerError, refusal.code != nil, !refusal.mayHaveHappened,
              refusal.revisions == nil else { return nil }
        return refusal
    }

    private func park(_ record: SubmittedCreation, refusal: ServerError) throws -> RefusedTaskDraft? {
        var parked = record
        parked.refusal = refusal
        guard let draft = parked.refusedDraft else { return nil }
        try write(parked)
        return draft
    }

    /// The Machine's receipt for `record`, sending it again only when the Machine
    /// says to continue; nil while another call owns it or after it was retired.
    public func reconcile(_ snapshot: SubmittedCreation, keepingRefusedDraft: Bool = false) async throws -> CreationLookup? {
        // Own recovery before the first await, including destination lookup.
        guard active.insert(snapshot.id).inserted else { return nil }
        defer { active.remove(snapshot.id) }
        // A receipt reads the same on every revision; only sending the saved
        // payload again needs the revision it was written for.
        let file = url(snapshot.slot)
        guard FileManager.default.fileExists(atPath: file.path) else { return nil }
        let record = try read(file)
        guard record.id == snapshot.id else { return nil }
        if record.refusal != nil {
            if keepingRefusedDraft, let draft = record.refusedDraft { throw draft }
            return nil
        }
        let known = try await receipt(for: record)
        guard FileManager.default.fileExists(atPath: file.path) else { return nil }
        if let known, known.disposition != .continue { return known }
        guard record.version == ServerProtocol.revision else {
            throw RegistryQueryError("This submitted creation requires protocol revision \(record.version); its request is retained")
        }
        do { _ = try await send(record.payload) }
        catch {
            guard keepingRefusedDraft, known == nil, let refusal = Self.refusal(error) else { throw error }
            if let receipt = try await receipt(for: record) { return receipt }
            guard let draft = try park(record, refusal: refusal) else { throw error }
            throw draft
        }
        guard let receipt = try await receipt(for: record) else {
            throw RegistryQueryError("The Machine has no receipt for creation \(record.id)")
        }
        return receipt
    }

    private func receipt(for record: SubmittedCreation) async throws -> CreationLookup? {
        guard try await currentMachine() == record.destination else {
            throw RegistryQueryError("Waiting for the original Machine \(record.destination)")
        }
        let receipt = try await lookup(record.id).decode(CreationLookup?.self)
        guard receipt == nil || receipt?.requestId == record.id else {
            throw RegistryQueryError("The Machine returned the receipt for another creation")
        }
        return receipt
    }
}
