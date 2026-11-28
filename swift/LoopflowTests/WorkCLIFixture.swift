#if os(macOS)
import Foundation
import Testing

struct WorkCLIFixture: Sendable {
    let root: URL
    let repo: URL
    let lf: BuiltLf

    init() throws {
        // The Machine and Desktop sockets both need short paths on macOS.
        root = URL(fileURLWithPath: "/private/tmp/work-cli-\(UUID().uuidString)")
        repo = root.appendingPathComponent("repo")
        lf = try BuiltLf(home: root, cwd: repo)
        try FileManager.default.createDirectory(at: repo, withIntermediateDirectories: true)
    }

    func prepare() async throws {
        _ = try await run(["set", "--global", "default_agent", "claude"])
        let graphs = repo.appendingPathComponent(".lf/graphs")
        try FileManager.default.createDirectory(at: graphs, withIntermediateDirectories: true)
        try Data("- cmd: graph show proof\n- interactive: {name: demo, id: review}\n".utf8)
            .write(to: graphs.appendingPathComponent("proof.yaml"))
        for args in [["init", "-b", "main"], ["config", "user.name", "Fixture Person"],
                     ["config", "user.email", "fixture@example.invalid"], ["add", "."], ["commit", "-m", "Fixture"]] {
            _ = try await run(args, executable: "/usr/bin/git")
        }
        let remote = root.appendingPathComponent("planning.git").path
        _ = try await run(["init", "--bare", remote], executable: "/usr/bin/git")
        _ = try await run(["remote", "add", "origin", remote], executable: "/usr/bin/git")
    }

    @MainActor
    func json(_ args: [String]) async throws -> [String: Any] {
        try #require(JSONSerialization.jsonObject(with: Data(try await run(args).utf8)) as? [String: Any])
    }

    func run(_ args: [String], executable: String? = nil) async throws -> String {
        try await lf.run(args, executable: executable)
    }
}
#endif
