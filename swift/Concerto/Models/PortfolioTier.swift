import Foundation

struct PortfolioTier: Codable, Hashable, Identifiable {
    let id: String
    var displayName: String

    var order: Int {
        Self.all.firstIndex { $0.id == id } ?? Self.default.order
    }

    static let all: [PortfolioTier] = [
        .init(id: "core", displayName: "Core"),
        .init(id: "active", displayName: "Active"),
        .init(id: "future", displayName: "Future"),
        .init(id: "deprecated", displayName: "Deprecated"),
    ]

    static let `default` = all[1]

    static func find(_ id: String) -> PortfolioTier {
        all.first { $0.id == id } ?? .default
    }
}
