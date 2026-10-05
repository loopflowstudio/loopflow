#if os(macOS)
import Foundation
import Testing
@testable import Loopflow
@testable import LoopflowMac

/// Owned synthetic native history and executable, prepared by desktop_performance.py.
/// The public CLI supplies Session identity and launch commands; no live Home is copied.
struct DesktopNativeSessionFixture: Decodable, Sendable {
    let cli: String
    let home: String
    let environment: [String: String]
    let sessionId: String
    let nativeId: String
    let transcript: String
    let history: String

    static func load() throws -> Self {
        let path = try #require(ProcessInfo.processInfo.environment["LOOPFLOW_TEST_NATIVE_FIXTURE"])
        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        return try decoder.decode(Self.self, from: Data(contentsOf: URL(fileURLWithPath: path)))
    }

    var query: RegistryQuery {
        RegistryQuery { args, _ in
            try await Task.detached { try capture([cli] + args) }.value
        }
    }

    func records() async throws -> [SessionRecord] {
        let output = try await Task.detached {
            try capture([cli, "session", "list", "--all", "--history", "--json"])
        }.value
        return try JSONDecoder().decode([SessionRecord].self, from: Data(output.utf8))
    }

    func resume(_ record: SessionRecord) async throws -> String {
        try await Task.detached { try capture(record.openArgv) }.value
    }

    func isolatedCommand(_ argv: [String]) -> [String] {
        ["/usr/bin/env", "-i"] + environment.sorted { $0.key < $1.key }.map { "\($0.key)=\($0.value)" } + argv
    }

    func historyIsPreserved() throws -> Bool {
        try String(contentsOfFile: transcript, encoding: .utf8) == history
    }

    private func capture(_ argv: [String]) throws -> String {
        let output = URL(fileURLWithPath: home).appendingPathComponent("query-\(UUID().uuidString)")
        _ = FileManager.default.createFile(atPath: output.path, contents: nil)
        let errors = output.appendingPathExtension("stderr")
        _ = FileManager.default.createFile(atPath: errors.path, contents: nil)
        let errorHandle = try FileHandle(forWritingTo: errors)
        let handle = try FileHandle(forWritingTo: output)
        defer {
            try? handle.close(); try? errorHandle.close()
            try? FileManager.default.removeItem(at: output)
            try? FileManager.default.removeItem(at: errors)
        }
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/env")
        process.arguments = argv
        process.environment = environment
        process.currentDirectoryURL = URL(fileURLWithPath: home)
        process.standardInput = FileHandle.nullDevice
        process.standardOutput = handle
        process.standardError = errorHandle
        try process.run()
        let deadline = ContinuousClock.now + .seconds(60)
        while process.isRunning && ContinuousClock.now < deadline {
            Thread.sleep(forTimeInterval: 0.01)
        }
        if process.isRunning {
            process.terminate()
            Thread.sleep(forTimeInterval: 0.1)
            if process.isRunning { kill(process.processIdentifier, SIGKILL) }
            process.waitUntilExit()
            throw RegistryQueryError("Native fixture CLI timed out")
        }
        guard process.terminationStatus == 0 else {
            let detail = try String(contentsOf: errors, encoding: .utf8)
            throw RegistryQueryError("Native fixture CLI failed (\(process.terminationStatus)): \(detail)")
        }
        return try String(contentsOf: output, encoding: .utf8)
    }
}
@Suite("Native Session reopen fixture", .serialized)
@MainActor
struct DesktopNativeSessionTests {
    @Test(.enabled(if: ProcessInfo.processInfo.environment["LOOPFLOW_TEST_NATIVE_FIXTURE"] != nil))
    func reopenPreservesSessionAndNativeHistory() async throws {
        let fixture = try DesktopNativeSessionFixture.load()
        let store = SessionsStore(repoPath: fixture.home, query: fixture.query)
        for _ in 0..<3 {
            store.reconcile(try await fixture.records())
            await store.select(fixture.sessionId)
            let prepared = try #require(store.sessions.first?.surface)
            #expect(prepared.id == fixture.sessionId)
            let result = try await fixture.resume(prepared)
            #expect(result.contains("native:\(fixture.nativeId):retained-history"))
            store.recordPaneLive(fixture.sessionId)
            store.noteSurfaceClosed(.session(fixture.sessionId))
            let records = try await fixture.records()
            #expect(records.map(\.id) == [fixture.sessionId])
            #expect(try fixture.historyIsPreserved())
        }
    }
}
#endif
