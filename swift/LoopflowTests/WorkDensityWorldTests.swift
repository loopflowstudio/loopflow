#if os(macOS)
import Foundation
import SwiftUI
import Testing
import ViewInspector
@testable import Loopflow
@testable import LoopflowMac

/// Reads a `density` world built by `lf-world` through the built CLI. Uses
/// `LOOPFLOW_TEST_LF`/`LOOPFLOW_TEST_LF_WORLD` or `target/debug/*`; the swift
/// gate suite builds both first.
@Suite("Sidebar density on a built world", .serialized)
@MainActor
struct WorkDensityWorldTests {
    @Test("At density the sidebar lists opened started Tasks and one row per Wave, never orphan Sessions")
    func sidebarDensity() async throws {
        let world = try await DensityWorld()
        defer { world.remove() }
        let model = WorkModel(query: world.query, repoPath: world.repo.path)
        await model.refresh()

        let roadmaps = model.visibleRoadmaps
        #expect(roadmaps.map(\.wave.name).sorted() == ["infrastructure", "intelligence", "product"])
        #expect(roadmaps.allSatisfy { $0.tasks.items.count == 50 })
        let product = try #require(roadmaps.first { $0.wave.name == "product" })
        let started = product.tasks.items.filter { $0.runtime?.started == true }
        #expect(started.count == 8)
        #expect(model.projection.unmatchedSessions.count == 4)
        let orphans = model.projection.unmatchedSessions.map(\.id)

        // Opening each started product Task retains it in the working list.
        for task in started { model.select(.task(id: task.id)) }
        let seventh = try #require(started.first { $0.task.name.hasSuffix(" 7") })
        model.select(.task(id: seventh.id))
        let outline = try WorkNavigator(model: model, onOpenSession: { _ in }).inspect()
        #expect(outline.findAll(ViewType.Button.self) {
            (try? $0.accessibilityIdentifier().hasPrefix("work-task-")) == true
        }.count == 8)
        for roadmap in roadmaps {
            _ = try outline.find(viewWithAccessibilityIdentifier: "work-wave-\(roadmap.wave.id)")
        }
        #expect(throws: (any Error).self) { try outline.find(WaveLensView.self) }
        for orphan in orphans {
            #expect(throws: (any Error).self) { try outline.find(viewWithAccessibilityIdentifier: "session-row-\(orphan)") }
        }

        // Binding every orphan leaves no unmatched Session behind.
        for orphan in orphans {
            _ = try await world.query.bindSession(id: orphan, taskId: seventh.task.identifier, cwd: world.repo.path)
        }
        await model.refreshSessions()
        #expect(model.projection.unmatchedSessions.isEmpty)
        #expect(model.taskSessions(seventh.id).count == 5)
    }
}

private struct DensityWorld {
    let root: URL
    let repo: URL
    let query: RegistryQuery

    init() async throws {
        let source = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().appendingPathComponent("target/debug")
        let inherited = ProcessInfo.processInfo.environment
        let lf = inherited["LOOPFLOW_TEST_LF"] ?? source.appendingPathComponent("lf").path
        let builder = inherited["LOOPFLOW_TEST_LF_WORLD"] ?? source.appendingPathComponent("lf-world").path
        try #require(FileManager.default.isExecutableFile(atPath: lf) && FileManager.default.isExecutableFile(atPath: builder),
                     "Build the source CLI with cargo build -p loopflow --bin lf --bin lf-world")
        root = FileManager.default.temporaryDirectory.appendingPathComponent("density-\(UUID().uuidString)")
            .resolvingSymlinksInPath()
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        try "[user]\n\tname = Demo\n\temail = demo@example.com\n"
            .write(to: root.appendingPathComponent(".gitconfig"), atomically: true, encoding: .utf8)
        let world = root.appendingPathComponent("world")
        repo = world.appendingPathComponent("repo")
        // Nothing from the launching agent, installed Machine or host terminal is inherited.
        let base = ["PATH": "/usr/bin:/bin:/usr/sbin:/sbin", "HOME": root.path, "LF_BIN": lf,
                    "GIT_CONFIG_NOSYSTEM": "1", "GIT_ALLOW_PROTOCOL": "file", "GIT_TERMINAL_PROMPT": "0"]
        let built = try await Self.run(builder, ["build", "density", world.path], cwd: root, environment: base)
        let worldEnvironment = try JSONDecoder().decode([String: String].self, from: Data(built.utf8))
        let environment = base.merging(worldEnvironment) { _, world in world }
        let repo = repo
        query = RegistryQuery { args, cwd in
            try await Self.run(lf, args, cwd: URL(fileURLWithPath: cwd ?? repo.path), environment: environment)
        }
    }

    func remove() { try? FileManager.default.removeItem(at: root) }

    private static func run(_ binary: String, _ args: [String], cwd: URL, environment: [String: String]) async throws -> String {
        try await Task.detached {
            let process = Process()
            process.executableURL = URL(fileURLWithPath: binary)
            process.arguments = args
            process.currentDirectoryURL = cwd
            process.environment = environment
            process.standardInput = FileHandle.nullDevice
            let output = Pipe()
            let errors = Pipe()
            process.standardOutput = output
            process.standardError = errors
            try process.run()
            // Drain stderr concurrently so a chatty build cannot fill its pipe.
            async let diagnostics = Task.detached { errors.fileHandleForReading.readDataToEndOfFile() }.value
            let data = output.fileHandleForReading.readDataToEndOfFile()
            let stderr = await diagnostics
            process.waitUntilExit()
            guard process.terminationStatus == 0 else {
                throw RegistryQueryError("\(args.joined(separator: " ")): \(String(decoding: stderr, as: UTF8.self))")
            }
            return String(decoding: data, as: UTF8.self)
        }.value
    }
}
#endif
