// Portfolio repository entry for persistence.

import Foundation
import LoopflowCore

struct PortfolioRepo: Codable, Identifiable, Hashable {
    let path: String
    var lastOpened: Date
    var tierId: String
    var priority: Double

    var id: String { path }
    var url: URL { URL(fileURLWithPath: path) }
    var displayName: String { url.lastPathComponent }

    init(path: String, lastOpened: Date, tierId: String, priority: Double) {
        self.path = path
        self.lastOpened = lastOpened
        self.tierId = tierId
        self.priority = priority
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        path = try container.decode(String.self, forKey: .path)
        lastOpened = try container.decode(Date.self, forKey: .lastOpened)
        tierId = try container.decodeIfPresent(String.self, forKey: .tierId) ?? PortfolioTier.default.id
        priority = try container.decodeIfPresent(Double.self, forKey: .priority)
            ?? -lastOpened.timeIntervalSinceReferenceDate
    }
}
