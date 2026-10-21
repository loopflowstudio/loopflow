#if os(macOS)
import Darwin
import Foundation
import Testing
import Loopflow
@testable import LoopflowMac

/// The source-built `lf` in a private Machine. `LOOPFLOW_TEST_LF` overrides
/// `target/debug/lf`; the swift gate suite builds it first. Nothing from the
/// launching agent, installed Machine or host terminal is inherited.
struct BuiltLf: Sendable {
    let binary: String
    let environment: [String: String]
    /// Where a command runs when its caller names no directory.
    let cwd: URL

    init(home: URL, cwd: URL? = nil, overriding: [String: String] = [:]) throws {
        binary = try Self.binary("lf")
        environment = ["PATH": "/usr/bin:/bin:/usr/sbin:/sbin", "HOME": home.path,
                       "LF_HOME": home.path, "LF_BIN": binary, "LF_USER_NAME": "Fixture Person",
                       "GIT_CONFIG_NOSYSTEM": "1", "GIT_CONFIG_GLOBAL": "/dev/null",
                       "GIT_ALLOW_PROTOCOL": "file", "GIT_TERMINAL_PROMPT": "0"]
            .merging(overriding) { $1 }
        self.cwd = cwd ?? home
    }

    /// `lf` or `lf-world` from `LOOPFLOW_TEST_LF`/`LOOPFLOW_TEST_LF_WORLD` or `target/debug`.
    static func binary(_ name: String) throws -> String {
        let variable = "LOOPFLOW_TEST_" + name.uppercased().replacingOccurrences(of: "-", with: "_")
        let path = ProcessInfo.processInfo.environment[variable]
            ?? URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
                .deletingLastPathComponent().appendingPathComponent("target/debug/\(name)").path
        try #require(FileManager.default.isExecutableFile(atPath: path),
                     "Build with cargo build -p loopflow --bin lf --bin lf-world")
        return path
    }

    /// Desktop's reads over this Machine, including its live Work stream.
    var query: RegistryQuery {
        RegistryQuery(runWithInput: { args, cwd, input in
            try await run(args, cwd: cwd.map(URL.init(fileURLWithPath:)), input: input)
        }, watchWork: {
            try LocalWorkObservation.start(process: process(["monitor", "work", "--watch", "--json"]),
                                           configurationChanged: { false })
        }) { args, cwd in
            try await run(args, cwd: cwd.map(URL.init(fileURLWithPath:)))
        }
    }

    func process(_ args: [String], cwd: URL? = nil, executable: String? = nil) -> Process {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: executable ?? binary)
        process.arguments = args
        process.currentDirectoryURL = cwd ?? self.cwd
        process.environment = environment
        return process
    }

    /// Captured stdout; a nonzero exit throws its stderr.
    func run(_ args: [String], cwd: URL? = nil, input: String? = nil,
             executable: String? = nil) async throws -> String {
        // Files rather than pipes: a large reading never blocks on a full pipe.
        let scratch = FileManager.default.temporaryDirectory.appendingPathComponent("built-lf-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: scratch, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: scratch) }
        let files = ["stdin", "stdout", "stderr"].map { scratch.appendingPathComponent($0) }
        try Data((input ?? "").utf8).write(to: files[0])
        for file in files.dropFirst() { try Data().write(to: file) }
        let handles = [try FileHandle(forReadingFrom: files[0]), try FileHandle(forWritingTo: files[1]),
                       try FileHandle(forWritingTo: files[2])]
        defer { for handle in handles { try? handle.close() } }
        let child = process(args, cwd: cwd, executable: executable)
        child.standardInput = input == nil ? FileHandle.nullDevice : handles[0]
        child.standardOutput = handles[1]
        child.standardError = handles[2]
        let status: Int32 = try await withCheckedThrowingContinuation { exited in
            child.terminationHandler = { exited.resume(returning: $0.terminationStatus) }
            do { try child.run() } catch {
                child.terminationHandler = nil
                exited.resume(throwing: error)
            }
        }
        guard status == 0 else {
            let detail = (try? String(contentsOf: files[2], encoding: .utf8)) ?? ""
            throw RegistryQueryError("\(([executable ?? "lf"] + args).joined(separator: " ")) exited \(status): \(detail)")
        }
        return try String(contentsOf: files[1], encoding: .utf8)
    }
}

/// A recipe built by the source-built `lf-world`, once per test process and
/// shared by every suite that reads it. Reads must not change it.
struct World: Sendable {
    /// Runs in the world's repository with the world's environment.
    let lf: BuiltLf
    var repo: URL { lf.cwd }
    var query: RegistryQuery { lf.query }

    static func shared(_ recipe: String) async throws -> World {
        try await builds.world(recipe)
    }

    private static let builds = Builds()

    private actor Builds {
        private var started: [String: Task<World, any Error>] = [:]

        func world(_ recipe: String) async throws -> World {
            if let build = started[recipe] { return try await build.value }
            let build = Task { try await World.build(recipe) }
            started[recipe] = build
            return try await build.value
        }
    }

    private static let prefix = "loopflow-world-"

    private static func build(_ recipe: String) async throws -> World {
        removeAbandonedWorlds()
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("\(prefix)\(recipe)-\(getpid())").resolvingSymlinksInPath()
        try? FileManager.default.removeItem(at: root)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        let builder = try BuiltLf(home: root)
        let printed = try await builder.run(["build", recipe, root.appendingPathComponent("world").path],
                                            executable: try BuiltLf.binary("lf-world"))
        let environment = try JSONDecoder().decode([String: String].self, from: Data(printed.utf8))
        let home = try #require(environment["LF_HOME"], "lf-world printed no LF_HOME")
        let repo = URL(fileURLWithPath: home).deletingLastPathComponent().appendingPathComponent("repo")
        return World(lf: try BuiltLf(home: root, cwd: repo, overriding: environment))
    }

    /// Worlds outlive their test process; remove those whose process has exited.
    private static func removeAbandonedWorlds() {
        let temporary = FileManager.default.temporaryDirectory
        for name in (try? FileManager.default.contentsOfDirectory(atPath: temporary.path)) ?? []
        where name.hasPrefix(prefix) {
            guard let pid = name.split(separator: "-").last.flatMap({ pid_t($0) }),
                  kill(pid, 0) != 0, errno == ESRCH else { continue }
            try? FileManager.default.removeItem(at: temporary.appendingPathComponent(name))
        }
    }
}
#endif
