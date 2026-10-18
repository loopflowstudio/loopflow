import Foundation

/// Rust resolves default membership; Desktop only arranges these references.
public struct ViewMember: Codable, Equatable, Sendable {
    public let work: WorkWorkspaceKey
    public let title: String

    public init(work: WorkWorkspaceKey, title: String) {
        self.work = work; self.title = title
    }
}

public struct ViewPlan: Codable, Equatable, Sendable {
    public let owner: WorkWorkspaceKey
    public let members: [ViewMember]

    public init(owner: WorkWorkspaceKey, members: [ViewMember]) {
        self.owner = owner; self.members = members
    }
}

public struct WorkFileTab: Codable, Equatable, Sendable, Identifiable {
    public let id: String
    public let path: String
    public init(id: String, path: String) {
        self.id = id; self.path = path
    }
}
