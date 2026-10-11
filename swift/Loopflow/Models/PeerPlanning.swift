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
    public let pendingLocal: Bool
    public let conflicts: [PeerProjectionConflict]
    public let changes: [String: PlanningMutation]
    public let candidates: Set<String>

    enum CodingKeys: String, CodingKey {
        case id, reference, active, conflicts, changes, candidates
        case selectedRecords = "selected_records"
        case importedRevision = "imported_revision"
        case fetchedRevision = "fetched_revision"
        case acquisitionError = "acquisition_error"
        case publicationRevision = "publication_revision"
        case publicationState = "publication_state"
        case publicationError = "publication_error"
        case pendingLocal = "pending_local"
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

/// The retained mutation itself, not a reconstructed Work record.
public struct PlanningMutation: Decodable, Sendable, Equatable {
    public struct Edit: Decodable, Sendable, Equatable {
        public struct Change: Decodable, Sendable, Equatable {
            public let field: String
            public let value: JSONValue
        }
        public let target: String
        public let change: Change
    }

    public let edit: Edit
    public let clock: Int64
    public let parents: Set<String>

    public var valueText: String {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys]
        guard let data = try? encoder.encode(edit.change.value) else { return "Value unavailable" }
        return String(decoding: data, as: UTF8.self)
    }
}
