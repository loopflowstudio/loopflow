import Foundation
import Testing
@testable import Concerto

@Suite("Portfolio Service")
struct PortfolioServiceTests {
    @Test("addRepo keeps most recently opened repo first and de-duplicates by path")
    func addRepoDeduplicatesAndSorts() throws {
        let (defaults, key, cleanup) = makeDefaults()
        defer { cleanup() }

        let root = try makeTempDirectory(prefix: "portfolio-service")
        let repoA = root.appendingPathComponent("repo-a", isDirectory: true)
        let repoB = root.appendingPathComponent("repo-b", isDirectory: true)
        try FileManager.default.createDirectory(at: repoA, withIntermediateDirectories: true)
        try FileManager.default.createDirectory(at: repoB, withIntermediateDirectories: true)

        let service = PortfolioService(defaults: defaults, key: key)
        service.addRepo(repoA)
        service.addRepo(repoB)
        service.addRepo(repoA)

        #expect(service.repos.count == 2)
        #expect(service.repos.first?.path == repoA.normalizedFilePath)
        #expect(service.repos.last?.path == repoB.normalizedFilePath)
    }

    @Test("removeRepo updates stored list")
    func removeRepo() throws {
        let (defaults, key, cleanup) = makeDefaults()
        defer { cleanup() }

        let root = try makeTempDirectory(prefix: "portfolio-remove")
        let repoA = root.appendingPathComponent("repo-a", isDirectory: true)
        let repoB = root.appendingPathComponent("repo-b", isDirectory: true)
        try FileManager.default.createDirectory(at: repoA, withIntermediateDirectories: true)
        try FileManager.default.createDirectory(at: repoB, withIntermediateDirectories: true)

        let service = PortfolioService(defaults: defaults, key: key)
        service.addRepo(repoA)
        service.addRepo(repoB)

        service.removeRepo(repoA)
        #expect(service.repos.count == 1)
        #expect(service.repos[0].path == repoB.normalizedFilePath)
    }

    private func makeDefaults() -> (UserDefaults, String, () -> Void) {
        let suiteName = "portfolio-tests-\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: suiteName)!
        let key = "portfolio.repos"

        let cleanup = {
            defaults.removePersistentDomain(forName: suiteName)
        }

        return (defaults, key, cleanup)
    }

    private func makeTempDirectory(prefix: String) throws -> URL {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("\(prefix)-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        return directory
    }
}
