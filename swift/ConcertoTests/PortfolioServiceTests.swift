import Foundation
import Testing
@testable import Concerto

@Suite("Portfolio Service")
struct PortfolioServiceTests {
    @Test("addRepo keeps most recently opened repo first and de-duplicates by path")
    func addRepoDeduplicatesAndSorts() throws {
        let (defaults, key, legacyKey, cleanup) = makeDefaults()
        defer { cleanup() }

        let root = try makeTempDirectory(prefix: "portfolio-service")
        let repoA = root.appendingPathComponent("repo-a", isDirectory: true)
        let repoB = root.appendingPathComponent("repo-b", isDirectory: true)
        try FileManager.default.createDirectory(at: repoA, withIntermediateDirectories: true)
        try FileManager.default.createDirectory(at: repoB, withIntermediateDirectories: true)

        let service = PortfolioService(defaults: defaults, key: key, legacyKey: legacyKey)
        service.addRepo(repoA)
        service.addRepo(repoB)
        service.addRepo(repoA)

        #expect(service.repos.count == 2)
        #expect(service.repos.first?.path == repoA.normalizedFilePath)
        #expect(service.repos.last?.path == repoB.normalizedFilePath)
    }

    @Test("loads existing repos from legacy recent key")
    func loadsLegacyKey() throws {
        let (defaults, key, legacyKey, cleanup) = makeDefaults()
        defer { cleanup() }

        let repo = try makeTempDirectory(prefix: "portfolio-legacy").appendingPathComponent("repo", isDirectory: true)
        try FileManager.default.createDirectory(at: repo, withIntermediateDirectories: true)

        let legacyRepo = PortfolioRepo(
            path: repo.normalizedFilePath,
            lastOpened: Date()
        )
        let data = try JSONEncoder().encode([legacyRepo])
        defaults.set(data, forKey: legacyKey)

        let service = PortfolioService(defaults: defaults, key: key, legacyKey: legacyKey)

        #expect(service.repos.count == 1)
        #expect(service.repos[0].path == legacyRepo.path)
        #expect(defaults.data(forKey: key) != nil)
    }

    @Test("removeRepo and clearAll update stored list")
    func removeAndClear() throws {
        let (defaults, key, legacyKey, cleanup) = makeDefaults()
        defer { cleanup() }

        let root = try makeTempDirectory(prefix: "portfolio-remove")
        let repoA = root.appendingPathComponent("repo-a", isDirectory: true)
        let repoB = root.appendingPathComponent("repo-b", isDirectory: true)
        try FileManager.default.createDirectory(at: repoA, withIntermediateDirectories: true)
        try FileManager.default.createDirectory(at: repoB, withIntermediateDirectories: true)

        let service = PortfolioService(defaults: defaults, key: key, legacyKey: legacyKey)
        service.addRepo(repoA)
        service.addRepo(repoB)

        service.removeRepo(repoA)
        #expect(service.repos.count == 1)
        #expect(service.repos[0].path == repoB.normalizedFilePath)

        service.clearAll()
        #expect(service.repos.isEmpty)
    }

    private func makeDefaults() -> (UserDefaults, String, String, () -> Void) {
        let suiteName = "portfolio-tests-\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: suiteName)!
        let key = "portfolio.repos"
        let legacyKey = "portfolio.legacy"

        let cleanup = {
            defaults.removePersistentDomain(forName: suiteName)
        }

        return (defaults, key, legacyKey, cleanup)
    }

    private func makeTempDirectory(prefix: String) throws -> URL {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("\(prefix)-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        return directory
    }
}
