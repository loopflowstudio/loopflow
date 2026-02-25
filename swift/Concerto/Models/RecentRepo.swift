// Portfolio repository entry for persistence.

import Foundation

struct PortfolioRepo: Codable, Identifiable, Hashable {
    let path: String
    var lastOpened: Date

    var id: String { path }
    var url: URL { URL(fileURLWithPath: path) }
    var displayName: String { url.lastPathComponent }
    var exists: Bool { FileManager.default.fileExists(atPath: path) }
}
