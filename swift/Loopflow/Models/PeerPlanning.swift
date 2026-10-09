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

    enum CodingKeys: String, CodingKey {
        case id, reference, active, conflicts
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
