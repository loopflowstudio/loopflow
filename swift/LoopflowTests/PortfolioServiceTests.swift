import Foundation
import Testing
import Loopflow
@testable import LoopflowMac

@MainActor
struct PortfolioServiceTests {
    @Test(.enabled(if: ProcessInfo.processInfo.environment["LOOPFLOW_PORTFOLIO_TEST_BIN"] != nil))
    func desktopWritesReadBackThroughCLI() async throws {
        let binary = try #require(ProcessInfo.processInfo.environment["LOOPFLOW_PORTFOLIO_TEST_BIN"])
        let home = FileManager.default.temporaryDirectory.appendingPathComponent("portfolio-cli-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: home, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: home) }
        let query = RegistryQuery { args, _ in
            try await Task.detached {
                let process = Process()
                process.executableURL = URL(fileURLWithPath: binary)
                process.arguments = args
                process.currentDirectoryURL = home
                var environment = ProcessInfo.processInfo.environment.filter { !$0.key.hasPrefix("LF_") }
                environment["LF_HOME"] = home.path
                environment["LF_BIN"] = binary
                process.environment = environment
                process.standardInput = FileHandle.nullDevice
                let output = Pipe()
                let errors = Pipe()
                process.standardOutput = output
                process.standardError = errors
                try process.run()
                let data = output.fileHandleForReading.readDataToEndOfFile()
                process.waitUntilExit()
                guard process.terminationStatus == 0 else {
                    throw RegistryQueryError(String(decoding: errors.fileHandleForReading.readDataToEndOfFile(), as: UTF8.self))
                }
                return String(decoding: data, as: UTF8.self)
            }.value
        }
        let service = PortfolioService(query: query)
        await service.refresh()
        #expect(service.selection?.configured == false)
        await service.perform(["import", home.appendingPathComponent("missing").path])
        let imported = try await query.portfolio()
        #expect(service.selection == imported)
        #expect(imported.mode == .manual)
        #expect(imported.repos.count == 1)
        #expect(imported.repos[0].error != nil)
        await service.perform(["remove", imported.repos[0].path])
        #expect(try await query.portfolio().repos.isEmpty)
        _ = try await query.portfolio(["folder", home.path])
        await service.refresh()
        #expect(service.selection?.mode == .folder)
        #expect(service.selection == (try await query.portfolio()))
    }

    @Test func delayedReadingCannotRestoreRemovedMembership() async {
        let transport = DelayedPortfolioReading()
        let model = WorkModel(query: RegistryQuery { _, _ in await transport.read() })
        let earlier = Task { await model.refreshPortfolio(initialRepoPath: nil) }
        while !(await transport.isWaiting) { await Task.yield() }
        await model.refreshPortfolio(initialRepoPath: nil)
        await transport.release()
        await earlier.value
        #expect(model.portfolioError == nil)
        #expect(model.repos.isEmpty)
    }

    @Test func sharedFixtureDecodes() throws {
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
        let data = try Data(contentsOf: root.appendingPathComponent("tests/fixtures/dto/portfolio.json"))
        let value = try JSONDecoder().decode(Portfolio.self, from: data)
        #expect(value.configured)
        #expect(value.mode == .manual)
        #expect(value.repos.count == 2)
        #expect(value.repos[1].error == "Repository unavailable")
        #expect(try JSONDecoder().decode(Portfolio.self, from: JSONEncoder().encode(value)) == value)
    }

    @Test func mutationsPublishSharedReadingAndFailureRetainsMembership() async throws {
        let state = PortfolioTransport()
        let service = PortfolioService(query: RegistryQuery { args, _ in try await state.run(args) })
        await service.refresh()
        #expect(service.repos.map(\.path) == ["/src/repo"])
        await service.perform(["manual"])
        #expect(service.selection?.mode == .manual)
        #expect(service.repos.isEmpty)
        await service.perform(["add", "/other/repo"])
        #expect(service.repos.map(\.path) == ["/other/repo"])
        await service.perform(["folder", "/missing"])
        #expect(service.selection?.mode == .folder)
        #expect(service.selection?.root == "/missing")
        #expect(service.error == "Folder unavailable")
        #expect(service.repos.map(\.path) == ["/other/repo"])
        await state.fail()
        await service.refresh()
        #expect(service.repos.map(\.path) == ["/other/repo"])
        #expect(service.error == "Unavailable")
    }

    @Test func legacyImportRequiresAnUnconfiguredSharedSelection() async throws {
        let suite = "portfolio-\(UUID().uuidString)"
        let defaults = try #require(UserDefaults(suiteName: suite))
        defer { defaults.removePersistentDomain(forName: suite) }
        let saved = Data("[{\"path\":\"/saved/repo\",\"lastOpened\":0}]".utf8)
        defaults.set(saved, forKey: "portfolioRepos")
        let state = PortfolioTransport()
        let service = PortfolioService(query: RegistryQuery { args, _ in try await state.run(args) }, defaults: defaults)
        #expect(service.legacyPaths.isEmpty)
        await service.refresh()
        #expect(service.legacyPaths == ["/saved/repo"])
        await service.importLegacy()
        #expect(service.repos.map(\.path) == ["/saved/repo"])
        #expect(service.legacyPaths.isEmpty)
        #expect(defaults.data(forKey: "portfolioRepos") == saved)
    }
}

private actor PortfolioTransport {
    private var configured = false
    private var mode = "folder"
    private var root = "/src"
    private var readingError: String?
    private var paths = ["/src/repo"]
    private var failed = false
    func fail() { failed = true }
    func run(_ args: [String]) throws -> String {
        if failed { throw RegistryQueryError("Unavailable") }
        if args.contains("manual") { configured = true; mode = "manual"; paths = [] }
        if args.count > 3, args[1] == "add" || args[1] == "import" {
            configured = true; mode = "manual"; paths = [args[2]]
        }
        if args.count > 3, args[1] == "folder" {
            configured = true; mode = "folder"; root = args[2]; paths = []; readingError = "Folder unavailable"
        }
        let json: [String: Any] = ["configured": configured, "mode": mode, "root": root,
                                  "repos": paths.map { ["path": $0, "error": NSNull()] as [String: Any] },
                                  "error": readingError as Any? ?? NSNull()]
        return String(decoding: try JSONSerialization.data(withJSONObject: json), as: UTF8.self)
    }
}

private actor DelayedPortfolioReading {
    private var continuation: CheckedContinuation<String, Never>?
    var isWaiting: Bool { continuation != nil }
    func read() async -> String {
        if continuation != nil {
            return #"{"configured":true,"mode":"manual","root":"/src","repos":[],"error":null}"#
        }
        return await withCheckedContinuation { continuation = $0 }
    }
    func release() {
        continuation?.resume(returning: #"{"configured":true,"mode":"manual","root":"/src","repos":[{"path":"/removed/repo","error":null}],"error":null}"#)
        continuation = nil
    }
}
