#if os(macOS)
import Foundation
import Testing
@testable import Loopflow
@testable import LoopflowMac

/// The Wave inventory and sidebar read a `lifecycle` world built by lf-world and
/// served by the source CLI; no row is written by hand.
@Suite("Lifecycle world", .serialized)
@MainActor
struct LifecycleWorldTests {
    @Test func waveInventoryShowsCurrentTasksAndHistoryFiltersCompletionsByRange() async throws {
        let world = try await LifecycleWorld()
        defer { try? FileManager.default.removeItem(at: world.root) }
        let model = await world.model()
        let tasks = try #require(model.visibleRoadmaps.first { $0.wave.name == "release" }).tasks.items
        let names = { (group: [RoadmapTask]) in Set(group.map(\.task.name)) }

        let current = WaveTaskGroups(tasks)
        // Owned work survives an upstream cancel until someone disposes of it.
        #expect(names(current.owned) == ["Paused mid-flight", "Canceled upstream", "Lost checkout", "In progress"])
        #expect(names(current.unowned) == ["Not started"])

        // Project history admits successful completions by range, never abandoned or canceled work.
        var filter = TaskHistoryFilter()
        filter.showCompleted = true
        let settled = tasks.filter { !$0.task.isCurrent($0.condition) }
        for (days, expected) in [("7", ["Shipped yesterday", "Shipped this week"]),
                                 ("30", ["Shipped yesterday", "Shipped this week", "Shipped last fortnight"]),
                                 ("0", ["Shipped yesterday", "Shipped this week", "Shipped last fortnight", "Shipped last quarter"])] {
            filter.editDays(days)
            let admitted = settled.filter { filter.includes($0.task, condition: $0.condition, now: Date()) }
            #expect(names(admitted) == Set(expected), "range \(days)")
        }

        #expect(Set(model.visibleWork.standaloneTasks.map(\.task.task.name)) == ["Standalone idea", "Standalone fix"])
    }

    @Test func sidebarRetainsSettledTasksAndHidesPausedOnes() async throws {
        let world = try await LifecycleWorld()
        defer { try? FileManager.default.removeItem(at: world.root) }
        let model = await world.model()
        let tasks = try #require(model.visibleRoadmaps.first { $0.wave.name == "release" }).tasks.items
        let id = { (name: String) in try #require(tasks.first { $0.task.name == name }?.id) }
        let retained = [try id("Shipped yesterday"), try id("Abandoned attempt"), try id("In progress")]
        let paused = try id("Paused mid-flight")
        for task in retained + [paused] { model.select(.task(id: task)) }

        #expect(Set(retained).isSubset(of: model.workingTaskIDs))
        #expect(!model.workingTaskIDs.contains(paused))
    }
}

/// One `lf-world build lifecycle` per test, with the CLI confined to its env.
private struct LifecycleWorld: Sendable {
    let root: URL
    let repo: URL
    let binary: URL
    let env: [String: String]

    init() async throws {
        root = FileManager.default.temporaryDirectory.appendingPathComponent("lifecycle-world-\(UUID().uuidString)")
            .resolvingSymlinksInPath()
        binary = ProcessInfo.processInfo.environment["LOOPFLOW_TEST_LF"].map(URL.init(fileURLWithPath:))
            ?? URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
                .deletingLastPathComponent().appendingPathComponent("target/debug/lf")
        let builder = binary.deletingLastPathComponent().appendingPathComponent("lf-world")
        try #require(FileManager.default.isExecutableFile(atPath: binary.path) && FileManager.default.isExecutableFile(atPath: builder.path),
                     "Build the source CLI with cargo build -p loopflow --bin lf --bin lf-world")
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        try Data("[user]\n\tname = Demo\n\temail = demo@example.com\n".utf8)
            .write(to: root.appendingPathComponent(".gitconfig"))
        let world = root.appendingPathComponent("world")
        repo = world.appendingPathComponent("repo")
        let built = try await Self.run(builder, ["build", "lifecycle", world.path], cwd: root,
                                       env: Self.base(root: root, binary: binary))
        let decoded = try JSONSerialization.jsonObject(with: Data(built.utf8)) as? [String: String]
        env = Self.base(root: root, binary: binary).merging(try #require(decoded)) { $1 }
    }

    /// A refreshed WorkModel reading this world through the source CLI.
    @MainActor func model() async -> WorkModel {
        let model = WorkModel(query: RegistryQuery { args, _ in try await Self.run(binary, args, cwd: repo, env: env) },
                              repoPath: repo.path)
        await model.refresh()
        return model
    }

    // Nothing from the launching agent, installed Machine, provider or account is inherited.
    private static func base(root: URL, binary: URL) -> [String: String] {
        ["PATH": "/usr/bin:/bin:/usr/sbin:/sbin", "HOME": root.path, "LF_BIN": binary.path,
         "GIT_CONFIG_NOSYSTEM": "1", "GIT_TERMINAL_PROMPT": "0"]
    }

    private static func run(_ executable: URL, _ args: [String], cwd: URL, env: [String: String]) async throws -> String {
        try await Task.detached {
            let process = Process()
            process.executableURL = executable
            process.arguments = args
            process.currentDirectoryURL = cwd
            process.environment = env
            process.standardInput = FileHandle.nullDevice
            let output = FileManager.default.temporaryDirectory.appendingPathComponent("lifecycle-world-output-\(UUID().uuidString)")
            let errors = output.appendingPathExtension("stderr")
            FileManager.default.createFile(atPath: output.path, contents: nil)
            FileManager.default.createFile(atPath: errors.path, contents: nil)
            defer {
                try? FileManager.default.removeItem(at: output)
                try? FileManager.default.removeItem(at: errors)
            }
            process.standardOutput = try FileHandle(forWritingTo: output)
            process.standardError = try FileHandle(forWritingTo: errors)
            try process.run()
            process.waitUntilExit()
            guard process.terminationStatus == 0 else {
                throw RegistryQueryError("\(executable.lastPathComponent) \(args): \(try String(contentsOf: errors, encoding: .utf8))")
            }
            return try String(contentsOf: output, encoding: .utf8)
        }.value
    }
}
#endif
