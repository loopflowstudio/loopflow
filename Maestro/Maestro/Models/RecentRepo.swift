// Recent repository entry for persistence.

import Foundation

struct RecentRepo: Codable, Identifiable {
    let path: String
    let lastOpened: Date

    var id: String { path }
    var url: URL { URL(fileURLWithPath: path) }
    var displayName: String { url.lastPathComponent }
    var exists: Bool { FileManager.default.fileExists(atPath: path) }
}
