import Foundation
import Loopflow

/// One disposable list reading, scoped to the verified Machine. Never stores
/// terminal output or pending commands, and never supplies a live subscription.
actor MobileTaskCache {
    struct Reading: Codable, Sendable {
        let machineID: String
        let receivedAt: Date
        let tasks: [WorkViewPlan.Content]
    }

    static let application = MobileTaskCache(url: URL.cachesDirectory.appending(path: "Loopflow/tasks.json"))
    static let maximumBytes = 2 * 1024 * 1024
    private let url: URL

    init(url: URL) { self.url = url }

    func load(machineID: String) throws -> Reading? {
        guard FileManager.default.fileExists(atPath: url.path) else { return nil }
        let size = try url.resourceValues(forKeys: [.fileSizeKey]).fileSize
        guard let size, size <= Self.maximumBytes else { try remove(); return nil }
        let data = try Data(contentsOf: url)
        guard let reading = try? JSONDecoder().decode(Reading.self, from: data) else {
            try remove()
            return nil
        }
        return reading.machineID == machineID ? reading : nil
    }

    func save(_ reading: Reading) throws {
        try Task.checkCancellation()
        let data = try JSONEncoder().encode(reading)
        guard data.count <= Self.maximumBytes else { try remove(); return }
        try FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
        try Task.checkCancellation()
        try data.write(to: url, options: [.atomic, .completeFileProtection])
        var excluded = url
        var values = URLResourceValues()
        values.isExcludedFromBackup = true
        try excluded.setResourceValues(values)
    }

    func remove(machineID: String) throws {
        guard try load(machineID: machineID) != nil else { return }
        try remove()
    }

    private func remove() throws {
        guard FileManager.default.fileExists(atPath: url.path) else { return }
        try FileManager.default.removeItem(at: url)
    }
}
