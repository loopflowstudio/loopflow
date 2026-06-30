import Foundation

struct PortfolioTier: Codable, Hashable, Identifiable {
    let id: String
    var displayName: String
    var order: Int

    static let all: [PortfolioTier] = [
        .init(id: "core", displayName: "Core", order: 0),
        .init(id: "active", displayName: "Active", order: 1),
        .init(id: "future", displayName: "Future", order: 2),
        .init(id: "deprecated", displayName: "Deprecated", order: 3),
    ]

    static let `default` = all[1]

    static func find(_ id: String) -> PortfolioTier {
        all.first { $0.id == id } ?? .default
    }
}
