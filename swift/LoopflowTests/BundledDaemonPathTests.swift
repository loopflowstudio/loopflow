// End-to-end checks that any process Loopflow spawns — the bundled lfd and the
// tmux invocations driving the terminal panel — can actually find user-installed
// binaries like tmux, which GUI-launched apps lose access to because their
// inherited PATH is /usr/bin:/bin:/usr/sbin:/sbin.

import Foundation
import Testing
@testable import LoopflowMac

@Suite("GUI Process Environment")
struct BundledDaemonPathTests {
    // macOS GUI apps inherit this when launched from the Dock or Finder.
    private static let guiPath = "/usr/bin:/bin:/usr/sbin:/sbin"

    @Test("tmux resolves under the enriched PATH a GUI-launched daemon would see")
    func tmuxResolvesUnderEnrichedPath() throws {
        guard isToolInstalledSomewhere("tmux") else { return }

        let enriched = GUIProcessEnvironment.enrichedPath(from: Self.guiPath)

        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/env")
        process.arguments = ["-i", "PATH=\(enriched)", "tmux", "-V"]

        let stdout = Pipe()
        let stderr = Pipe()
        process.standardOutput = stdout
        process.standardError = stderr
        try process.run()
        process.waitUntilExit()

        let output = String(
            data: stdout.fileHandleForReading.readDataToEndOfFile(),
            encoding: .utf8
        ) ?? ""

        #expect(process.terminationStatus == 0)
        #expect(output.contains("tmux"))
    }

    // Reproduces the exact failure mode the terminal panel hit in production:
    // Loopflow's TmuxSession spawns `/usr/bin/env tmux ...` with the GUI-minimal
    // environment, so `env` can't find tmux and exits 127. With
    // GUIProcessEnvironment.enriched applied, the same invocation succeeds.
    @Test("env tmux works with enriched env, fails with the bare GUI env")
    func envTmuxReproducesAndFixes() throws {
        guard isToolInstalledSomewhere("tmux") else { return }

        let guiEnv = ["PATH": Self.guiPath]

        let bare = try spawnAndWait(
            "/usr/bin/env",
            args: ["tmux", "-V"],
            env: guiEnv
        )
        #expect(bare.exit == 127, "expected env to fail to resolve tmux under the GUI PATH alone")
        #expect(bare.stderr.contains("tmux"), "expected stderr to mention tmux; got: \(bare.stderr)")

        let enriched = try spawnAndWait(
            "/usr/bin/env",
            args: ["tmux", "-V"],
            env: GUIProcessEnvironment.enriched(guiEnv)
        )
        #expect(enriched.exit == 0, "tmux should resolve under enriched env; stderr: \(enriched.stderr)")
        #expect(enriched.stdout.contains("tmux"))
    }

    @Test("enrichment is idempotent and preserves existing PATH entries")
    func enrichmentIsIdempotent() {
        let once = GUIProcessEnvironment.enrichedPath(from: Self.guiPath)
        let twice = GUIProcessEnvironment.enrichedPath(from: once)

        #expect(once == twice)
        #expect(once.contains("/usr/bin"))
        #expect(once.contains("/opt/homebrew/bin"))
    }

    // The regression that motivated asking the login shell: codex installs via
    // npm into a version-manager directory (~/.nvm/versions/node/<v>/bin) that
    // no fixed candidate list can name. Under the GUI PATH alone, spawning the
    // codex app-server failed with ENOENT and every wave mind died on startup.
    //
    // Asserted against whatever the user's shell actually reports, so the test
    // says something true on any machine: every directory the interactive login
    // shell has, an enriched GUI child has too.
    @Test("enriched PATH covers every directory the user's login shell provides")
    func enrichedPathCoversLoginShellDirectories() throws {
        guard let shellPath = interactiveLoginShellPath() else { return }

        let enriched = Set(
            GUIProcessEnvironment.enrichedPath(from: Self.guiPath)
                .split(separator: ":").map(String.init)
        )

        for dir in shellPath.split(separator: ":").map(String.init) {
            #expect(enriched.contains(dir), "login shell PATH entry missing from enriched PATH: \(dir)")
        }
    }

    // Independent of GUIProcessEnvironment's own shell plumbing — if both were
    // broken the same way, the test above would still pass vacuously.
    private func interactiveLoginShellPath() -> String? {
        let shell = ProcessInfo.processInfo.environment["SHELL"] ?? "/bin/zsh"
        guard FileManager.default.isExecutableFile(atPath: shell) else { return nil }

        guard let result = try? spawnAndWait(
            shell,
            args: ["-ilc", "printf '%s' \"$PATH\""],
            env: ["PATH": Self.guiPath, "HOME": FileManager.default.homeDirectoryForCurrentUser.path]
        ), result.exit == 0, !result.stdout.isEmpty else { return nil }

        return result.stdout
    }

    // Reproduces the Ghostty failure mode: once a surface is opened, Ghostty
    // spawns `/usr/bin/login -flp ... /bin/bash --noprofile --norc -c 'exec -l tmux ...'`.
    // The `--noprofile --norc` flags mean bash doesn't resource any login files,
    // so PATH comes entirely from Loopflow's own process environment. Under the
    // bare GUI PATH, `exec -l tmux` fails with "tmux: not found"; under our
    // enriched env it resolves.
    @Test("bash --noprofile --norc -c 'exec tmux' works with enriched env, fails with bare GUI env")
    func ghosttyStyleBashExecReproducesAndFixes() throws {
        guard isToolInstalledSomewhere("tmux") else { return }

        let guiEnv = ["PATH": Self.guiPath]

        let bare = try spawnAndWait(
            "/bin/bash",
            args: ["--noprofile", "--norc", "-c", "exec tmux -V"],
            env: guiEnv
        )
        #expect(bare.exit != 0, "expected bare GUI env to fail under Ghostty-style bash invocation")

        let enriched = try spawnAndWait(
            "/bin/bash",
            args: ["--noprofile", "--norc", "-c", "exec tmux -V"],
            env: GUIProcessEnvironment.enriched(guiEnv)
        )
        #expect(enriched.exit == 0, "tmux should resolve under enriched env; stderr: \(enriched.stderr)")
        #expect(enriched.stdout.contains("tmux"))
    }

    @Test("enriched env preserves non-PATH keys and upgrades PATH")
    func enrichedPreservesOtherKeys() {
        let input = ["PATH": Self.guiPath, "HOME": "/Users/example", "CUSTOM": "value"]
        let result = GUIProcessEnvironment.enriched(input)

        #expect(result["HOME"] == "/Users/example")
        #expect(result["CUSTOM"] == "value")
        #expect(result["PATH"]?.contains("/opt/homebrew/bin") == true)
    }

    private func isToolInstalledSomewhere(_ tool: String) -> Bool {
        let home = FileManager.default.homeDirectoryForCurrentUser.path
        let candidates = [
            "\(home)/.local/bin",
            "/opt/homebrew/bin",
            "/usr/local/bin",
            "/usr/bin",
        ]
        return candidates.contains {
            FileManager.default.isExecutableFile(atPath: "\($0)/\(tool)")
        }
    }

    private struct SpawnResult {
        let exit: Int32
        let stdout: String
        let stderr: String
    }

    private func spawnAndWait(
        _ executable: String,
        args: [String],
        env: [String: String]
    ) throws -> SpawnResult {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: executable)
        process.arguments = args
        process.environment = env

        let out = Pipe()
        let err = Pipe()
        process.standardOutput = out
        process.standardError = err

        try process.run()
        process.waitUntilExit()

        return SpawnResult(
            exit: process.terminationStatus,
            stdout: String(data: out.fileHandleForReading.readDataToEndOfFile(), encoding: .utf8) ?? "",
            stderr: String(data: err.fileHandleForReading.readDataToEndOfFile(), encoding: .utf8) ?? ""
        )
    }
}
