#if os(macOS)
import Foundation
import Testing
@testable import Loopflow
@testable import LoopflowMac

/// One `lf-world build <recipe>` served by the built CLI. Uses
/// `LOOPFLOW_TEST_LF`/`LOOPFLOW_TEST_LF_WORLD` or `target/debug/*`; the swift
/// gate suite builds both first.
struct BuiltWorld {
    let root: URL
    let repo: URL
    let query: RegistryQuery

    init(recipe: String) async throws {
        let source = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().appendingPathComponent("target/debug")
        let inherited = ProcessInfo.processInfo.environment
        let lf = inherited["LOOPFLOW_TEST_LF"] ?? source.appendingPathComponent("lf").path
        let builder = inherited["LOOPFLOW_TEST_LF_WORLD"] ?? source.appendingPathComponent("lf-world").path
        try #require(FileManager.default.isExecutableFile(atPath: lf) && FileManager.default.isExecutableFile(atPath: builder),
                     "Build the source CLI with cargo build -p loopflow --bin lf --bin lf-world")
        root = FileManager.default.temporaryDirectory.appendingPathComponent("\(recipe)-\(UUID().uuidString)")
            .resolvingSymlinksInPath()
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        try "[user]\n\tname = Demo\n\temail = demo@example.com\n"
            .write(to: root.appendingPathComponent(".gitconfig"), atomically: true, encoding: .utf8)
        let world = root.appendingPathComponent("world")
        repo = world.appendingPathComponent("repo")
        // Nothing from the launching agent, installed Machine or host terminal is inherited.
        let base = ["PATH": "/usr/bin:/bin:/usr/sbin:/sbin", "HOME": root.path, "LF_BIN": lf,
                    "GIT_CONFIG_NOSYSTEM": "1", "GIT_ALLOW_PROTOCOL": "file", "GIT_TERMINAL_PROMPT": "0"]
        let built = try await Self.run(builder, ["build", recipe, world.path], cwd: root, environment: base)
        let worldEnvironment = try JSONDecoder().decode([String: String].self, from: Data(built.utf8))
        let environment = base.merging(worldEnvironment) { _, world in world }
        let repo = repo
        query = RegistryQuery { args, cwd in
            try await Self.run(lf, args, cwd: URL(fileURLWithPath: cwd ?? repo.path), environment: environment)
        }
    }

    /// A refreshed WorkModel reading this world.
    @MainActor func model() async -> WorkModel {
        let model = WorkModel(query: query, repoPath: repo.path)
        await model.refresh()
        return model
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
