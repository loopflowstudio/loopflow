// Portfolio repository entry for persistence.

import Foundation
import Loopflow

struct PortfolioRepo: Codable, Identifiable, Hashable, Sendable {
    let path: String
    var lastOpened: Date

    var id: String { path }
    var url: URL { URL(fileURLWithPath: path) }
    var displayName: String { url.lastPathComponent }
}
