import Foundation
import Testing
@testable import Concerto

@Suite("Portfolio Service")
struct PortfolioServiceTests {
    @Test("addRepo places new repos at the top of Active and preserves existing rank")
    func addRepoRanksNewReposAtTopOfActive() throws {
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

        #expect(service.orderedRepos.count == 2)
        #expect(service.orderedRepos[0].path == repoB.normalizedFilePath)
        #expect(service.orderedRepos[1].path == repoA.normalizedFilePath)
        #expect(service.orderedRepos.allSatisfy { $0.tierId == PortfolioTier.default.id })
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
        #expect(service.orderedRepos.count == 1)
        #expect(service.orderedRepos[0].path == repoB.normalizedFilePath)
    }

    @Test("legacy stored repos decode into Active without data loss")
    func legacyReposDecodeIntoActive() throws {
        let (defaults, key, cleanup) = makeDefaults()
        defer { cleanup() }

        let root = try makeTempDirectory(prefix: "portfolio-legacy")
        let repoA = root.appendingPathComponent("repo-a", isDirectory: true)
        let repoB = root.appendingPathComponent("repo-b", isDirectory: true)
        try FileManager.default.createDirectory(at: repoA, withIntermediateDirectories: true)
        try FileManager.default.createDirectory(at: repoB, withIntermediateDirectories: true)

        let older = Date(timeIntervalSinceReferenceDate: 100)
        let newer = Date(timeIntervalSinceReferenceDate: 200)
        let legacy = [
            LegacyPortfolioRepo(path: repoA.normalizedFilePath, lastOpened: older),
            LegacyPortfolioRepo(path: repoB.normalizedFilePath, lastOpened: newer),
        ]
        defaults.set(try JSONEncoder().encode(legacy), forKey: key)

        let service = PortfolioService(defaults: defaults, key: key)

        #expect(service.orderedRepos.map(\.path) == [repoB.normalizedFilePath, repoA.normalizedFilePath])
        #expect(service.orderedRepos.allSatisfy { $0.tierId == PortfolioTier.default.id })
    }

    @Test("reposByTier returns all tiers in fixed order")
    func reposByTierIncludesEmptyTiers() throws {
        let (defaults, key, cleanup) = makeDefaults()
        defer { cleanup() }

        let root = try makeTempDirectory(prefix: "portfolio-tiers")
        let repo = root.appendingPathComponent("repo-a", isDirectory: true)
        try FileManager.default.createDirectory(at: repo, withIntermediateDirectories: true)

        let service = PortfolioService(defaults: defaults, key: key)
        service.addRepo(repo)

        let grouped = service.reposByTier()
        #expect(grouped.map(\.tier.id) == PortfolioTier.all.map(\.id))
        #expect(grouped.first { $0.tier.id == PortfolioTier.default.id }?.repos.count == 1)
        #expect(grouped.first { $0.tier.id == "core" }?.repos.isEmpty == true)
    }

    @Test("reorder assigns midpoint and edge priorities and persists")
    func reorderUpdatesRankAndPersists() throws {
        let (defaults, key, cleanup) = makeDefaults()
        defer { cleanup() }

        let root = try makeTempDirectory(prefix: "portfolio-reorder")
        let repoA = root.appendingPathComponent("repo-a", isDirectory: true)
        let repoB = root.appendingPathComponent("repo-b", isDirectory: true)
        let repoC = root.appendingPathComponent("repo-c", isDirectory: true)
        try FileManager.default.createDirectory(at: repoA, withIntermediateDirectories: true)
        try FileManager.default.createDirectory(at: repoB, withIntermediateDirectories: true)
        try FileManager.default.createDirectory(at: repoC, withIntermediateDirectories: true)

        let service = PortfolioService(defaults: defaults, key: key)
        service.addRepo(repoA)
        service.addRepo(repoB)
        service.addRepo(repoC)

        let active = PortfolioTier.default
        let ordered = service.orderedRepos
        service.reorder(repoA.normalizedFilePath, into: active, above: ordered[0], below: ordered[1])
        #expect(service.orderedRepos.map(\.path) == [
            repoC.normalizedFilePath,
            repoA.normalizedFilePath,
            repoB.normalizedFilePath,
        ])

        let future = PortfolioTier.find("future")
        service.reorder(repoB.normalizedFilePath, into: future, above: nil, below: nil)
        #expect(service.reposByTier().first { $0.tier.id == future.id }?.repos.map(\.path) == [
            repoB.normalizedFilePath,
        ])

        let activeRepos = service.reposByTier().first { $0.tier.id == active.id }?.repos ?? []
        service.reorder(repoB.normalizedFilePath, into: active, above: nil, below: activeRepos.first)
        #expect(service.orderedRepos.map(\.path) == [
            repoB.normalizedFilePath,
            repoC.normalizedFilePath,
            repoA.normalizedFilePath,
        ])

        let reloaded = PortfolioService(defaults: defaults, key: key)
        #expect(reloaded.orderedRepos.map(\.path) == [
            repoB.normalizedFilePath,
            repoC.normalizedFilePath,
            repoA.normalizedFilePath,
        ])
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

private struct LegacyPortfolioRepo: Encodable {
    let path: String
    let lastOpened: Date
}
