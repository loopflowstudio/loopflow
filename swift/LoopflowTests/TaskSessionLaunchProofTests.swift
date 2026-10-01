#if os(macOS) && SWIFT_PACKAGE && canImport(GhosttyKit)
import AppKit
import Foundation
import GhosttyKit
import SwiftUI
import Testing
import ViewInspector
@testable import Loopflow
@testable import LoopflowMac

private final class SessionLaunchBundleMarker: NSObject {}

@Suite("Task Session preparation subprocess proof", .requiresDisplay, .serialized)
@MainActor
struct TaskSessionLaunchProofTests {
    @Test("Delayed preparation and launch failures stay with the requesting Task; retry uses its prepared checkout")
    func delayedPreparationKeepsItsTask() async throws {
        // SwiftPM's main bundle is its SDK helper. Re-enter this one test in
        // an owned copy, retaining SwiftPM's framework search environment.
        // The development configuration below must never touch the SDK.
        if ProcessInfo.processInfo.environment["LOOPFLOW_PREPARATION_TEST_HOST"] == nil {
            let host = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
            try FileManager.default.createDirectory(at: host, withIntermediateDirectories: true)
            defer { try? FileManager.default.removeItem(at: host) }
            let executable = try #require(Bundle.main.executableURL)
            try #require(executable.lastPathComponent == "swiftpm-testing-helper")
            let owned = host.appendingPathComponent(executable.lastPathComponent)
            try FileManager.default.copyItem(at: executable, to: owned)
            try JSONSerialization.data(withJSONObject: ["lf_path": host.appendingPathComponent("fixture/lf").path])
                .write(to: host.appendingPathComponent("LoopflowDevControl.json"))
            let process = Process()
            process.executableURL = owned
            process.arguments = ["--test-bundle-path", try #require(Bundle(for: SessionLaunchBundleMarker.self).executablePath),
                                 "--testing-library", "swift-testing",
                                 "--filter", "TaskSessionLaunchProofTests", "--no-parallel"]
            process.environment = ProcessInfo.processInfo.environment.merging(
                ["LOOPFLOW_PREPARATION_TEST_HOST": host.path], uniquingKeysWith: { _, new in new })
            let output = Pipe()
            process.standardOutput = output
            process.standardError = output
            try process.run()
            let log = output.fileHandleForReading.readDataToEndOfFile()
            process.waitUntilExit()
            print(String(decoding: log, as: UTF8.self))
            try #require(process.terminationStatus == 0)
            try #require(FileManager.default.fileExists(atPath: host.appendingPathComponent("proof-complete").path))
            return
        }
        _ = NSApplication.shared
        GhosttyManager.shared.initialize()
        let fm = FileManager.default
        let ownedHost = try #require(ProcessInfo.processInfo.environment["LOOPFLOW_PREPARATION_TEST_HOST"])
        let directory = URL(fileURLWithPath: ownedHost).appendingPathComponent("fixture")
        let checkout = directory.appendingPathComponent("prepared task")
        try fm.createDirectory(at: checkout, withIntermediateDirectories: true)
        defer { try? fm.removeItem(at: directory) }

        // Configure only the generated SwiftPM test bundle, through the same
        // explicit helper configuration used by a development app.
        let resources = try #require(Bundle.main.resourceURL)
        try #require(resources.resolvingSymlinksInPath().path == URL(fileURLWithPath: ownedHost).resolvingSymlinksInPath().path)
        let config = resources.appendingPathComponent("LoopflowDevControl.json")
        let original = try? Data(contentsOf: config)
        defer {
            if let original { try? original.write(to: config) }
            else { try? fm.removeItem(at: config) }
        }
        let helper = directory.appendingPathComponent("lf")
        let script = #"""
        #!/bin/sh
        fixture_dir=$(dirname "$0")
        if [ "$1" = "task" ] && [ "$2" = "checkout" ]; then
            printf '%s\n' "$@" > "$fixture_dir/prepare-args"
            pwd > "$fixture_dir/prepare-cwd"
            attempt=0
            while [ ! -f "$fixture_dir/release" ]; do
                attempt=$((attempt + 1))
                [ "$attempt" -lt 500 ] || exit 9
                sleep 0.02
            done
            mode=$(cat "$fixture_dir/mode")
            if [ "$mode" = "reject" ]; then
                printf 'Preparation refused for the requested Task\n' >&2
                exit 7
            fi
            if [ "$mode" = "missing-helper" ]; then chmod -x "$0"; fi
            cat "$fixture_dir/receipt.json"
        elif [ "$1" = "--interactive" ]; then
            printf '%s\n' "$@" > "$fixture_dir/conversation-args"
            pwd > "$fixture_dir/conversation-cwd"
            exec /bin/cat
        else
            exit 8
        fi
        """#
        try script.write(to: helper, atomically: true, encoding: .utf8)
        try fm.setAttributes([.posixPermissions: 0o755], ofItemAtPath: helper.path)
        try JSONSerialization.data(withJSONObject: ["lf_path": helper.path]).write(to: config)
        try #require(try LocalWaveAgentLauncher.controlLfPath() == helper.path)
        try JSONSerialization.data(withJSONObject: ["worktree": checkout.path]).write(
            to: directory.appendingPathComponent("receipt.json"))

        let fixture = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/dto/roadmap_snapshot.json")
        var plan = try #require(JSONSerialization.jsonObject(with: Data(contentsOf: fixture)) as? [String: Any])
        var waves = try #require(plan["waves"] as? [[String: Any]])
        for wi in waves.indices {
            var wave = try #require(waves[wi]["wave"] as? [String: Any])
            if wave["repo"] as? String == "/src/loopflow" { wave["repo"] = directory.path }
            waves[wi]["wave"] = wave
            var evidence = try #require(waves[wi]["tasks"] as? [String: Any])
            guard var items = evidence["items"] as? [[String: Any]] else { continue }
            for ti in items.indices {
                var reference = try #require(items[ti]["reference"] as? [String: Any])
                reference["workspace"] = NSNull()
                items[ti]["reference"] = reference
            }
            evidence["items"] = items; waves[wi]["tasks"] = evidence
        }
        plan["waves"] = waves
        let roadmap = String(decoding: try JSONSerialization.data(withJSONObject: plan), as: UTF8.self)
        let query = RegistryQuery { args, _ in
            switch args.first {
            case "roadmap": return roadmap
            case "wave" where args.dropFirst().first == "list": return "[]"
            case "session", "flow": return "[]"
            case "activity": return #"{"generated_at":1,"since":0,"limit":50,"truncated":false,"items":[]}"#
            case "pm": return #"{"identifier":"W2-131","comments":[]}"#
            default: throw RegistryQueryError("Unexpected read in preparation proof")
            }
        }
        let repo = directory.path
        let model = PodiumModel(query: query, repoPath: repo)
        await model.refresh()
        let origin = model.navigation
        let registry = SessionsWorkspaceRegistry()
        let workspace = registry.workspace(for: repo)
        workspace.multiplexer.newShell()
        let pane = workspace.multiplexer.focusedPaneId
        let terminal = registry.surfaces.view(for: .shell(pane))
        terminal.frame = CGRect(x: 0, y: 0, width: 600, height: 350)
        terminal.workingDirectory = repo
        terminal.command = buildWorkspaceShellCommand(id: pane, argv: ["/bin/cat"], env: [:])
        terminal.createSurface(manager: GhosttyManager.shared)
        let surface = try #require(terminal.surface)
        defer {
            for path in registry.paths {
                for pane in registry.workspace(for: path).multiplexer.layout.allPanes {
                    registry.surfaces.release(.shell(pane.id))
                }
            }
        }
        let layout = workspace.multiplexer.layout
        let draft = "preparation-keeps-my-draft"
        draft.withCString { ghostty_surface_text(surface, $0, UInt(draft.utf8.count)) }
        let view = SessionsView(model: model, repoPath: repo, workspaces: registry, query: query)
        let window = NSWindow(contentRect: CGRect(x: 0, y: 0, width: 1400, height: 900),
                              styleMask: [.titled], backing: .buffered, defer: false)
        window.contentView = NSHostingView(rootView: view)
        defer { window.contentView = nil }

        for mode in ["reject", "missing-helper", "success"] {
            try? fm.removeItem(at: directory.appendingPathComponent("release"))
            try? fm.removeItem(at: directory.appendingPathComponent("prepare-args"))
            try mode.write(to: directory.appendingPathComponent("mode"), atomically: true, encoding: .utf8)
            try fm.setAttributes([.posixPermissions: 0o755], ofItemAtPath: helper.path)
            model.select(.task(id: "issue-review"))
            try await settle(window)
            try view.inspect().find(viewWithAccessibilityIdentifier: "task-new-session").button().tap()
            try await wait { fm.fileExists(atPath: directory.appendingPathComponent("prepare-args").path) }
            #expect(origin.startingTaskSessions.contains("issue-review"))
            #expect(try String(contentsOf: directory.appendingPathComponent("prepare-args"), encoding: .utf8)
                == "task\ncheckout\nW2-131\n--json\n")
            if mode != "success" {
                model.select(.task(id: "issue-available"))
                try await settle(window)
                #expect(try !view.inspect().find(viewWithAccessibilityIdentifier: "task-new-session").button().isDisabled())
                model.setRepoPath(checkout.path)
            }
            try Data().write(to: directory.appendingPathComponent("release"))
            try await wait { !origin.startingTaskSessions.contains("issue-review") }
            if mode != "success" {
                #expect(model.navigation.taskSessionErrors.isEmpty)
                let error = try #require(origin.taskSessionErrors["issue-review"])
                #expect(error.contains(mode == "reject" ? "Preparation refused" : "missing or not executable"))
                #expect(!fm.fileExists(atPath: directory.appendingPathComponent("conversation-args").path))
                model.setRepoPath(repo)
                #expect(workspace.multiplexer.layout == layout)
            } else {
                #expect(origin.taskSessionErrors.isEmpty)
                #expect(model.selection == .task(id: "issue-review"))
                #expect(origin.preparedTaskWorktrees["issue-review"] == checkout.path)
                #expect(try view.inspect().find(viewWithAccessibilityIdentifier: "task-worktree-location")
                    .text().string() == checkout.lastPathComponent)
                try await wait { fm.fileExists(atPath: directory.appendingPathComponent("conversation-args").path) }
                let args = try String(contentsOf: directory.appendingPathComponent("conversation-args"), encoding: .utf8)
                #expect(args.hasPrefix("--interactive\n--task\nW2-131\n:\n"))
                let cwd = try String(contentsOf: directory.appendingPathComponent("conversation-cwd"), encoding: .utf8)
                    .trimmingCharacters(in: .whitespacesAndNewlines)
                #expect(URL(fileURLWithPath: cwd).resolvingSymlinksInPath().path == checkout.resolvingSymlinksInPath().path)
            }
            #expect(terminal.surface == surface)
        }
        registry.layout(for: repo).select(repo)
        model.navigation.content = .terminals
        try await settle(window)
        ghostty_surface_text(surface, "\n", 1)
        try await wait { terminalText(surface).components(separatedBy: draft).count >= 3 }
        #expect(terminal.surface == surface)
        try Data().write(to: URL(fileURLWithPath: ownedHost).appendingPathComponent("proof-complete"))
    }

    private func wait(_ condition: () -> Bool) async throws {
        let deadline = ContinuousClock.now + .seconds(5)
        while !condition(), ContinuousClock.now < deadline { try await Task.sleep(for: .milliseconds(20)) }
        try #require(condition())
    }

    private func settle(_ window: NSWindow) async throws {
        window.contentView?.layoutSubtreeIfNeeded()
        window.layoutIfNeeded()
        try await Task.sleep(for: .milliseconds(100))
    }

    private func terminalText(_ surface: ghostty_surface_t) -> String {
        var text = ghostty_text_s()
        let selection = ghostty_selection_s(
            top_left: ghostty_point_s(tag: GHOSTTY_POINT_SCREEN, coord: GHOSTTY_POINT_COORD_TOP_LEFT, x: 0, y: 0),
            bottom_right: ghostty_point_s(tag: GHOSTTY_POINT_SCREEN, coord: GHOSTTY_POINT_COORD_BOTTOM_RIGHT, x: 0, y: 0), rectangle: false)
        guard ghostty_surface_read_text(surface, selection, &text) else { return "" }
        defer { ghostty_surface_free_text(surface, &text) }
        guard let bytes = text.text else { return "" }
        return String(decoding: Data(bytes: bytes, count: Int(text.text_len)), as: UTF8.self)
    }
}
#endif
