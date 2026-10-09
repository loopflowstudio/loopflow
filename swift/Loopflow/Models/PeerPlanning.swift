import Foundation

/// The common Git planning status; publication and Linear delivery are separate.
public struct PeerPlanningStatus: Decodable, Sendable, Equatable, Identifiable {
    public let id: String
    public let reference: String
    public let active: Bool
    public let selectedRecords: UInt64
    public let importedRevision: String?
    public let fetchedRevision: String?
    public let acquisitionError: String?
    public let publicationRevision: String?
    public let publicationState: String?
    public let publicationError: String?
    public let pendingLocal: Bool?
    public let localError: String?
    public let conflicts: [PeerProjectionConflict]
    public let records: [PeerPlanningRecord]
    public let recoveryError: String?

    enum CodingKeys: String, CodingKey {
        case id, reference, active, conflicts, records
        case recoveryError = "recovery_error"
        case selectedRecords = "selected_records"
        case importedRevision = "imported_revision"
        case fetchedRevision = "fetched_revision"
        case acquisitionError = "acquisition_error"
        case publicationRevision = "publication_revision"
        case publicationState = "publication_state"
        case publicationError = "publication_error"
        case pendingLocal = "pending_local"
        case localError = "local_error"
    }
}

public struct PeerProjectionConflict: Decodable, Sendable, Equatable {
    public struct Object: Decodable, Sendable, Equatable {
        public let kind: String
        public let id: String
    }

    public let object: Object
    public let reason: String
}

/// Read-only local recovery; these records are never a publication payload.
public struct PeerPlanningRecord: Decodable, Sendable, Equatable, Identifiable {
    public let object: PeerProjectionConflict.Object
    public let localID: String
    public let destination: String?
    public let references: [PeerProjectionConflict.Object]
    public let values: [PeerPlanningValue]
    public var id: String { "\(object.kind):\(object.id)" }

    enum CodingKeys: String, CodingKey {
        case object, destination, references, values
        case localID = "local_id"
    }
}

public struct PeerPlanningValue: Decodable, Sendable, Equatable, Identifiable {
    public let id: String
    public let field: String
    public let valueJSON: String
    public let candidate: Bool
    public let author: TaskCommentAuthor?
    public let observedAt: Int64?

    enum CodingKeys: String, CodingKey {
        case id, field, candidate, author
        case valueJSON = "value_json"
        case observedAt = "observed_at"
    }

    public var authorLabel: String {
        switch author {
        case let .person(name): name ?? "Unknown person"
        case .integration: "Integration"
        case nil: "Unknown"
        }
    }
}
