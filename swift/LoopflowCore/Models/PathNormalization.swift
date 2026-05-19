// Canonical filesystem path normalization shared across repo-scoped views.

import Foundation

extension URL {
    public var normalizedFilePath: String {
        let path = standardizedFileURL.path(percentEncoded: false)
        // standardizedFileURL fixes `.`/`..` and percent-encoding but keeps
        // a trailing slash, so `/repo/` and `/repo` compare unequal. Repo
        // paths cross the lfd boundary in both forms; drop the trailing
        // separator (except root) so every comparison site agrees.
        if path.count > 1, path.hasSuffix("/") {
            return String(path.dropLast())
        }
        return path
    }
}

extension String {
    public var normalizedFilePath: String {
        URL(fileURLWithPath: self).normalizedFilePath
    }
}
