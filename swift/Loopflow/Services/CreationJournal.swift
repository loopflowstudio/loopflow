import Foundation

/// A creation saved before its first send: the exact payload, and the Machine it is for.
public struct SubmittedCreation: Codable, Sendable, Identifiable {
    public let requestID: String
    public let destination: String
    public let payload: Data
    public let version: Int
    public let submittedAt: Date
    public var id: String { requestID }
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
    private let lookup: @Sendable (String) async throws -> String
    private let send: @Sendable (Data) async throws -> String

    /// `lookup` prints `creation.get` for a request ID; `send` makes a submitted creation.
    public init(directory: URL,
                machine: @escaping @Sendable () async throws -> String,
                lookup: @escaping @Sendable (String) async throws -> String,
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

    public func save(_ record: SubmittedCreation) throws {
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let file = url(record.id)
        if FileManager.default.fileExists(atPath: file.path) {
            let existing = try JSONDecoder().decode(SubmittedCreation.self, from: Data(contentsOf: file))
            guard existing.payload == record.payload, existing.destination == record.destination else {
                throw RegistryQueryError("This creation identity already has another submitted request")
            }
            return
        }
        try JSONEncoder().encode(record).write(to: file, options: .atomic)
    }

    public func pending() throws -> [SubmittedCreation] {
        guard FileManager.default.fileExists(atPath: directory.path) else { return [] }
        return try FileManager.default.contentsOfDirectory(at: directory, includingPropertiesForKeys: nil)
            .filter { $0.pathExtension == "json" }
            .map { try JSONDecoder().decode(SubmittedCreation.self, from: Data(contentsOf: $0)) }
            .sorted { $0.submittedAt < $1.submittedAt }
    }

    public func presented(_ id: String) async throws {
        let file = url(id)
        guard FileManager.default.fileExists(atPath: file.path) else { return }
        let record = try JSONDecoder().decode(SubmittedCreation.self, from: Data(contentsOf: file))
        guard try await currentMachine() == record.destination else {
            throw RegistryQueryError("Waiting for the original Machine \(record.destination)")
        }
        try Task.checkCancellation()
        if FileManager.default.fileExists(atPath: file.path) { try FileManager.default.removeItem(at: file) }
    }

    /// Saves `change` under `requestID` for its Machine; saving it again is a no-op.
    public func record(_ change: ServedWrite, requestID: String) async throws -> SubmittedCreation {
        let record = SubmittedCreation(requestID: requestID, destination: try await currentMachine(),
            payload: try JSONEncoder().encode(change), version: ServerProtocol.revision, submittedAt: Date())
        try save(record)
        return record
    }

    public func submit(_ change: ServedWrite, requestID: String) async throws -> String {
        let record = try await record(change, requestID: requestID)
        active.insert(requestID)
        defer { active.remove(requestID) }
        // Saving precedes the first send. A lost answer leaves this exact record.
        do { return try await send(record.payload) }
        catch { throw SubmittedCreationError(requestID: requestID, cause: error) }
    }

    /// The Machine's receipt for `record`, sending it again only when the Machine
    /// says to continue; nil while another call is already sending it.
    public func reconcile(_ record: SubmittedCreation) async throws -> CreationLookup? {
        // Own recovery before the first await, including destination lookup.
        guard active.insert(record.id).inserted else { return nil }
        defer { active.remove(record.id) }
        guard try await currentMachine() == record.destination else {
            throw RegistryQueryError("Waiting for the original Machine \(record.destination)")
        }
        // A receipt reads the same on every revision; only sending the saved
        // payload again needs the revision it was written for.
        var receipt = try await receipt(record.id)
        if receipt == nil || receipt?.disposition == .continue {
            guard record.version == ServerProtocol.revision else {
                throw RegistryQueryError("This submitted creation requires protocol revision \(record.version); its request is retained")
            }
            _ = try await send(record.payload)
            receipt = try await self.receipt(record.id)
            guard receipt != nil else { throw RegistryQueryError("The Machine has no receipt for creation \(record.id)") }
        }
        return receipt
    }

    private func receipt(_ id: String) async throws -> CreationLookup? {
        let receipt = try JSONDecoder().decode(CreationLookup?.self, from: Data(try await lookup(id).utf8))
        guard receipt == nil || receipt?.requestId == id else {
            throw RegistryQueryError("The Machine returned the receipt for another creation")
        }
        return receipt
    }
}
