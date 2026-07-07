// Canonical filesystem path normalization shared across repo-scoped views.

import Foundation

extension URL {
    public var normalizedFilePath: String {
        standardizedFileURL.path(percentEncoded: false)
    }
}

extension String {
    public var normalizedFilePath: String {
        URL(fileURLWithPath: self).normalizedFilePath
    }
}
