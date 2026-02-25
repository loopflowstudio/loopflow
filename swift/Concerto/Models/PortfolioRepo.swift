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

extension URL {
    var normalizedFilePath: String {
        standardizedFileURL.path(percentEncoded: false)
    }
}

extension String {
    var normalizedFilePath: String {
        URL(fileURLWithPath: self).normalizedFilePath
    }
}
