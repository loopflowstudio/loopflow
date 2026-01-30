// RecentAreasService - persists recent area selections per repo in UserDefaults.

import Foundation

struct RecentAreasService {
    private let maxEntries = 5

    private func key(for repoURL: URL) -> String {
        let hash = repoURL.path.data(using: .utf8)?.hashValue ?? 0
        return "recentAreas.\(hash)"
    }

    func recentAreas(for repoURL: URL) -> [String] {
        let key = key(for: repoURL)
        return UserDefaults.standard.stringArray(forKey: key) ?? []
    }

    func addRecentArea(_ area: String, for repoURL: URL) {
        let key = key(for: repoURL)
        var areas = recentAreas(for: repoURL)

        // Remove existing if present
        areas.removeAll { $0 == area }

        // Insert at front
        areas.insert(area, at: 0)

        // Trim to max
        if areas.count > maxEntries {
            areas = Array(areas.prefix(maxEntries))
        }

        UserDefaults.standard.set(areas, forKey: key)
    }

    func clearRecentAreas(for repoURL: URL) {
        let key = key(for: repoURL)
        UserDefaults.standard.removeObject(forKey: key)
    }
}
