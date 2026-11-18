import Foundation

/// Fetch, import and publication status for one Git planning ref.
public struct GitDestination: Decodable, Sendable, Equatable, Identifiable {
    public let id: String
    public let reference: String
    public let active: Bool
    public let selectedRecords: UInt64
    public let importedRevision: String?
    public let fetchedRevision: String?
    public let fetchError: String?
    public let publicationRevision: String?
    public let publicationState: String?
    public let publicationError: String?
    public let pendingLocal: Bool
    public let conflicts: [GitDestinationConflict]

    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        id = try values.decode(String.self, forKey: .id)
        reference = try values.decode(String.self, forKey: .reference)
        active = try values.decode(Bool.self, forKey: .active)
        selectedRecords = try values.decode(UInt64.self, forKey: .selectedRecords)
        importedRevision = try values.decode(String?.self, forKey: .importedRevision)
        fetchedRevision = try values.decode(String?.self, forKey: .fetchedRevision)
        fetchError = try values.decode(String?.self, forKey: .fetchError)
        publicationRevision = try values.decode(String?.self, forKey: .publicationRevision)
        publicationState = try values.decode(String?.self, forKey: .publicationState)
        publicationError = try values.decode(String?.self, forKey: .publicationError)
        pendingLocal = try values.decode(Bool.self, forKey: .pendingLocal)
        conflicts = try values.decode([GitDestinationConflict].self, forKey: .conflicts)
    }

    enum CodingKeys: String, CodingKey {
        case id, reference, active, conflicts
        case selectedRecords = "selected_records"
        case importedRevision = "imported_revision"
        case fetchedRevision = "fetched_revision"
        case fetchError = "fetch_error"
        case publicationRevision = "publication_revision"
        case publicationState = "publication_state"
        case publicationError = "publication_error"
        case pendingLocal = "pending_local"
    }
}

public struct GitDestinationConflict: Decodable, Sendable, Equatable {
    public let target: String
    public let reason: String
}
