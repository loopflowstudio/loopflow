import Foundation
import Testing
import Loopflow
@testable import LoopflowMac

/// The built CLI resolves Work addresses; production Swift routing consumes them.
/// Run with `LOOPFLOW_WORK_OPEN_TEST_BIN=<target/debug/lf> scripts/test_desktop.sh --filter WorkOpeningCLITests`.
@MainActor
struct WorkOpeningCLITests {
    nonisolated private static let binary = ProcessInfo.processInfo.environment["LOOPFLOW_WORK_OPEN_TEST_BIN"]

    @Test(.enabled(if: binary != nil))
    func sameNamedRepositoriesOpenOnlyWhenQualified() async throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent("work-open-\(UUID().uuidString)")
        defer { try? FileManager.default.removeItem(at: root) }
        let home = root.appendingPathComponent("home")
        try FileManager.default.createDirectory(at: home, withIntermediateDirectories: true)
        let query = Self.query(binary: try #require(Self.binary), home: home)
        var identities: [(path: String, id: String)] = []
        for parent in ["one", "two"] {
            let repo = root.appendingPathComponent(parent).appendingPathComponent("alpha")
            try FileManager.default.createDirectory(at: repo, withIntermediateDirectories: true)
            try Self.git(["init", "-q", "-b", "main"], in: repo)
            try Self.git(["-c", "user.name=T", "-c", "user.email=t@e", "commit", "-q", "--allow-empty", "-m", "init"], in: repo)
            let identity = try await query.repositoryIdentity(cwd: repo.path)
            identities.append((repo.path, identity.id))
        }

        let router = WorkLinkRouter()
        await #expect(throws: RegistryQueryError.self) {
            _ = try await query.workLink(URL(string: "loopflow://alpha")!)
        }
        #expect(router.inspect().openings.isEmpty, "an ambiguous address changes nothing")

        let chosen = identities[1]
        let address = "loopflow://alpha?repository=\(chosen.id)"
        let resolved = try await query.workLink(URL(string: address)!)
        #expect(resolved.url == address)
        #expect(resolved.target.kind == .repository)
        #expect(LinkedWork(resolved) == nil, "a repository root delivers no Task or Wave")
        var opened: [RepositoryWorkspace] = []
        let workspace = try await router.openRepository(path: resolved.target.repositoryPath, link: LinkedWork(resolved),
                                                        address: resolved.url, query: query) { opened.append($0) }
        #expect(opened == [workspace])
        #expect(workspace.id == chosen.id)
        #expect(workspace.path.normalizedFilePath == URL(fileURLWithPath: chosen.path).resolvingSymlinksInPath().path.normalizedFilePath)
        #expect(router.inspect().openings.map(\.url) == [address])
    }

    private static func query(binary: String, home: URL) -> RegistryQuery {
        RegistryQuery { args, cwd in
            try await Task.detached {
                let process = Process()
                process.executableURL = URL(fileURLWithPath: binary)
                process.arguments = args
                process.currentDirectoryURL = URL(fileURLWithPath: cwd ?? home.path)
                var environment = ProcessInfo.processInfo.environment.filter {
                    !$0.key.hasPrefix("LF_") && !$0.key.hasPrefix("CMUX_") && !$0.key.hasPrefix("HERDR_")
                }
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
    }

    private static func git(_ args: [String], in directory: URL) throws {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/git")
        process.arguments = args
        process.currentDirectoryURL = directory
        try process.run()
        process.waitUntilExit()
        guard process.terminationStatus == 0 else { throw RegistryQueryError("git \(args.joined(separator: " ")) failed") }
    }
}
