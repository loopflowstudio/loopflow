import Foundation
import Testing
import Loopflow
@testable import LoopflowMac

/// The built CLI resolves Work addresses; production Swift routing consumes them.
/// Uses `LOOPFLOW_TEST_LF` or `target/debug/lf`; the swift gate suite builds it first.
@Suite("Work addresses from the built CLI", .serialized)
@MainActor
struct WorkOpeningCLITests {
    @Test func sameNamedRepositoriesOpenOnlyWhenQualified() async throws {
        let fixture = try Fixture()
        defer { fixture.remove() }
        var identities: [(path: String, id: String)] = []
        for parent in ["one", "two"] {
            let repo = try fixture.repository(parent: parent, name: "alpha")
            let identity = try await fixture.query.repositoryIdentity(cwd: repo.path)
            identities.append((repo.path, identity.id))
        }

        let router = WorkLinkRouter()
        await #expect(throws: RegistryQueryError.self) {
            _ = try await fixture.query.workLink(URL(string: "loopflow://alpha")!)
        }
        #expect(router.inspect().openings.isEmpty, "an ambiguous address changes nothing")

        let chosen = identities[1]
        let address = "loopflow://alpha?repository=\(chosen.id)"
        let resolved = try await fixture.query.workLink(URL(string: address)!)
        #expect(resolved.url == address)
        #expect(resolved.target.kind == .repository)
        #expect(LinkedWork(resolved) == nil, "a repository root delivers no Task or Wave")
        var opened: [RepositoryWorkspace] = []
        let workspace = try await router.openRepository(path: resolved.target.repositoryPath, link: LinkedWork(resolved),
                                                        address: resolved.url, query: fixture.query) { opened.append($0) }
        #expect(opened == [workspace])
        #expect(workspace.id == chosen.id)
        #expect(workspace.path.normalizedFilePath == URL(fileURLWithPath: chosen.path).resolvingSymlinksInPath().path.normalizedFilePath)
        #expect(router.inspect().openings.map(\.url) == [address])
    }

    /// Registered only through the CLI: the Task and its Wave open in that
    /// repository's one window, the Task as its conversation.
    @Test func taskAndWaveAddressesDeliverToTheirRepositoryWindow() async throws {
        let fixture = try Fixture()
        defer { fixture.remove() }
        let repo = try fixture.repository(parent: "work", name: "beta", planningRemote: true)
        let created = try await fixture.json(["task", "create", "--wave", "proof", "--title", "Arrives through CLI", "--json"], cwd: repo)
        let identifier = try #require(created["identifier"] as? String)

        let router = WorkLinkRouter()
        var windows: [RepositoryWorkspace] = []
        var received: [LinkedWork] = []
        for address in ["loopflow://beta/task/\(identifier)", "loopflow://beta/wave/proof"] {
            let resolved = try await fixture.query.workLink(URL(string: address)!)
            #expect(resolved.url == address)
            let link = try #require(LinkedWork(resolved))
            let workspace = try await router.openRepository(path: resolved.target.repositoryPath, link: link,
                                                            address: resolved.url, query: fixture.query) { windows.append($0) }
            if !router.hasWindow(workspace.id) {
                router.register(UUID(), repository: workspace.id, openingRequests: router.workspaceRequests(workspace.id),
                                focus: {}, inspect: Self.inspection, controlPane: { $0.target.pane },
                                readText: { _ in throw RegistryQueryError("No terminal") }) { received.append($0) }
            }
        }
        while received.count < 2 { await Task.yield() }
        #expect(windows.count == 1, "both open in the repository's one window")
        guard case .task(let task) = received[0] else { Issue.record("Expected the Task first"); return }
        #expect(task.issue == identifier)
        #expect(task.conversation, "an external Task link opens its conversation")
        #expect(task.address == "loopflow://beta/task/\(identifier)")
        #expect(received[1] == .wave(id: "proof", address: "loopflow://beta/wave/proof"))
    }

    private static func inspection(_ repository: String, _ window: UUID) -> DesktopWindowInspection {
        DesktopWindowInspection(repository: repository, window: window.uuidString, path: nil,
            selectionKind: nil, selectionId: nil, reading: "loading", reason: nil,
            task: nil, session: nil, supportedOperations: ["list"], workspaces: [], opening: nil)
    }

    private struct Fixture {
        let root: URL
        let home: URL
        let binary: String
        let query: RegistryQuery

        init() throws {
            root = FileManager.default.temporaryDirectory.appendingPathComponent("work-open-\(UUID().uuidString)")
                .resolvingSymlinksInPath()
            home = root.appendingPathComponent("home")
            let binary = ProcessInfo.processInfo.environment["LOOPFLOW_TEST_LF"]
                ?? URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
                    .deletingLastPathComponent().appendingPathComponent("target/debug/lf").path
            try #require(FileManager.default.isExecutableFile(atPath: binary), "Build the source CLI with cargo build -p loopflow --bin lf")
            self.binary = binary
            try FileManager.default.createDirectory(at: home, withIntermediateDirectories: true)
            let home = home
            query = RegistryQuery { args, cwd in
                try await Self.run(binary, args, cwd: URL(fileURLWithPath: cwd ?? home.path), home: home)
            }
        }

        func remove() { try? FileManager.default.removeItem(at: root) }

        func repository(parent: String, name: String, planningRemote: Bool = false) throws -> URL {
            let repo = root.appendingPathComponent(parent).appendingPathComponent(name)
            try FileManager.default.createDirectory(at: repo, withIntermediateDirectories: true)
            try Self.git(["init", "-q", "-b", "main"], in: repo)
            try Self.git(["-c", "user.name=T", "-c", "user.email=t@e", "commit", "-q", "--allow-empty", "-m", "init"], in: repo)
            if planningRemote {
                let remote = root.appendingPathComponent("\(name)-planning.git").path
                try Self.git(["init", "-q", "--bare", remote], in: repo)
                try Self.git(["remote", "add", "origin", remote], in: repo)
            }
            return repo
        }

        func json(_ args: [String], cwd: URL) async throws -> [String: Any] {
            let output = try await Self.run(binary, args, cwd: cwd, home: home)
            return try #require(JSONSerialization.jsonObject(with: Data(output.utf8)) as? [String: Any])
        }

        private static func run(_ binary: String, _ args: [String], cwd: URL, home: URL) async throws -> String {
            try await Task.detached {
                let process = Process()
                process.executableURL = URL(fileURLWithPath: binary)
                process.arguments = args
                process.currentDirectoryURL = cwd
                // Nothing from the launching agent, installed Machine or host terminal is inherited.
                process.environment = ["PATH": "/usr/bin:/bin:/usr/sbin:/sbin", "HOME": home.path,
                                       "LF_HOME": home.path, "LF_BIN": binary, "LF_USER_NAME": "Fixture Person",
                                       "GIT_CONFIG_NOSYSTEM": "1", "GIT_CONFIG_GLOBAL": "/dev/null",
                                       "GIT_ALLOW_PROTOCOL": "file", "GIT_TERMINAL_PROMPT": "0"]
                process.standardInput = FileHandle.nullDevice
                let output = Pipe()
                let errors = Pipe()
                process.standardOutput = output
                process.standardError = errors
                try process.run()
                let data = output.fileHandleForReading.readDataToEndOfFile()
                let diagnostics = errors.fileHandleForReading.readDataToEndOfFile()
                process.waitUntilExit()
                guard process.terminationStatus == 0 else {
                    throw RegistryQueryError(String(decoding: diagnostics, as: UTF8.self))
                }
                return String(decoding: data, as: UTF8.self)
            }.value
        }

        private static func git(_ args: [String], in directory: URL) throws {
            let process = Process()
            process.executableURL = URL(fileURLWithPath: "/usr/bin/git")
            process.arguments = args
            process.currentDirectoryURL = directory
            process.environment = ["PATH": "/usr/bin:/bin", "GIT_CONFIG_NOSYSTEM": "1", "GIT_CONFIG_GLOBAL": "/dev/null"]
            try process.run()
            process.waitUntilExit()
            guard process.terminationStatus == 0 else { throw RegistryQueryError("git \(args.joined(separator: " ")) failed") }
        }
    }
}
