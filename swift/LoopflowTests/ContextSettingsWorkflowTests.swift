#if os(macOS)
import Foundation
import Loopflow
import SwiftUI
import Testing
import ViewInspector
@testable import LoopflowMac

@MainActor
struct ContextSettingsWorkflowTests {
    @Test func checkoutDiffFromSettingsToLaunch() async throws {
        let root = URL(fileURLWithPath: "/tmp/lf-diff-\(UUID().uuidString.prefix(8))").resolvingSymlinksInPath()
        let home = root.appending(path: "home")
        let repo = root.appending(path: "checkout")
        let linked = root.appending(path: "linked")
        let bin = root.appending(path: "bin")
        let record = root.appending(path: "launch.json")
        let fm = FileManager.default
        for dir in [home, repo.appending(path: ".lf/skills"), bin] {
            try fm.createDirectory(at: dir, withIntermediateDirectories: true)
        }
        defer { try? fm.removeItem(at: root) }

        let source = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        let standIn = source.appending(path: "rust/loopflow-execution/src/harness/testdata/pi_rpc.py")
        let pi = bin.appending(path: "pi")
        try ("#!/usr/bin/python3\n" + String(contentsOf: standIn, encoding: .utf8)).write(to: pi, atomically: true, encoding: .utf8)
        try fm.setAttributes([.posixPermissions: 0o755], ofItemAtPath: pi.path)
        let lf = try BuiltLf(home: home, cwd: repo, overriding: [
            "PATH": bin.path + ":/usr/bin:/bin", "PI_FIXTURE_RECORD": record.path
        ])
        func git(_ args: String...) async throws {
            _ = try await lf.run(Array(args), executable: "/usr/bin/git")
        }
        let shared = repo.appending(path: ".lf/config.yaml")
        let mine = repo.appending(path: ".lf/config.local.yaml")
        let original = "diff: true\ncontext: [README.md] # included at launch\n"
        try original.write(to: shared, atomically: true, encoding: .utf8)
        try Data("Read the context.\n".utf8).write(to: repo.appending(path: ".lf/skills/probe.md"))
        try Data("Checkout documentation.\n".utf8).write(to: repo.appending(path: "README.md"))
        try Data("pi-conversation.jsonl\n.lf/*.lock\n.lf/prompts/\n".utf8).write(to: repo.appending(path: ".gitignore"))
        try await git("init", "-b", "main")
        try await git("add", ".")
        try await git("-c", "user.name=Fixture", "-c", "user.email=fixture@example.org", "commit", "-m", "Checkout")
        let origin = root.appending(path: "origin.git")
        try await git("init", "--bare", origin.path)
        try await git("remote", "add", "origin", origin.path)
        try await git("push", "-u", "origin", "main")
        try await git("switch", "-c", "fixture")
        try Data("Changed documentation.\n".utf8).write(to: repo.appending(path: "README.md"))

        try await git("add", "README.md")
        try await git("-c", "user.name=Fixture", "-c", "user.email=fixture@example.org", "commit", "-m", "Change documentation")
        try await git("worktree", "add", "-b", "linked", linked.path)

        func launchHasDiff(in checkout: URL) async throws -> Bool {
            if fm.fileExists(atPath: record.path) { try fm.removeItem(at: record) }
            _ = try await lf.run(["-b", "skill", "probe", "-a", "pi"], cwd: checkout)
            let captured = try #require(JSONSerialization.jsonObject(with: Data(contentsOf: record)) as? [String: Any])
            let prompt = try #require(captured["input"] as? String)
            #expect(prompt.contains("Changed documentation."))
            #expect(prompt.contains("Read the context."))
            let context = try #require(captured["context"] as? String)
            return context.contains("-changes.md")
        }
        func cliScope(_ directory: URL) async throws -> ConfigurationScope {
            let text = try await lf.run(["config", "scope", "--json"], cwd: directory)
            return try JSONDecoder().decode(ConfigurationScope.self, from: Data(text.utf8))
        }

        // Desktop opens an installed Machine. Finish first-open migrations before connecting.
        _ = try await lf.run(["desktop", "portfolio", "--json"])
        let server = LocalServerFixture(lf)
        let model = ContextSettingsModel(location: server.location())
        let notices = NoticeCenter()
        model.checkout = repo.path
        var watch = Task { await model.observe(repo.path) }
        func awaitReading(_ condition: () -> Bool, sourceLocation: SourceLocation = #_sourceLocation) async throws {
            let deadline = ContinuousClock.now + .seconds(10)
            while model.saving || !condition() {
                try #require(model.reading.failure == nil, "\(model.reading.failure ?? "")", sourceLocation: sourceLocation)
                try #require(ContinuousClock.now < deadline, "Context subscription last showed \(String(describing: model.file?.declared)); edit error: \(model.editError ?? "none")", sourceLocation: sourceLocation)
                try await Task.sleep(for: .milliseconds(20))
            }
        }

        do {
            _ = try await server.status()
            try await awaitReading { model.file != nil }
            #expect(model.file?.diff.isEmpty == true)
            let view = ContextSettings(model: model, notices: notices)
            let toggle = try view.inspect().find(ViewType.Toggle.self)
            #expect(try toggle.isOn())
            #expect(try await launchHasDiff(in: repo))
            try toggle.tap()
            try await awaitReading { model.file?.declared == .bool(false) }
            #expect(try await !launchHasDiff(in: repo))

            let picker = try view.inspect().find(ViewType.Picker.self)
            try picker.select(value: ConfigurationDestination.sharedRepository)
            #expect(try view.inspect().find(ViewType.Toggle.self).isOn())
            let sharedFile = try #require(model.file)
            #expect(sharedFile.resetPreview == "- diff: true")
            try view.inspect().find(ViewType.Toggle.self).tap()
            try await awaitReading { model.file?.declared == .bool(false) }
            #expect(try String(contentsOf: shared, encoding: .utf8) == original.replacingOccurrences(of: "true", with: "false"))
            try view.inspect().find(ViewType.Toggle.self).tap()
            try await awaitReading { model.file?.declared == .bool(true) }
            #expect(model.reading.value?.source == ConfigurationDestination.personalRepository.title)
            #expect(try await !launchHasDiff(in: repo))

            try picker.select(value: ConfigurationDestination.personalRepository)
            let privateOff = try #require(model.file)
            await model.apply(.reset, to: privateOff, notices: notices, manager: nil)
            try await awaitReading { model.file?.diff.isEmpty == true }
            #expect(try await launchHasDiff(in: repo))
            let undo = try #require(notices.current?.action)
            try picker.select(value: ConfigurationDestination.sharedRepository)
            undo.perform()
            try await awaitReading { model.reading.value?.contextDiff.value == .bool(false) }
            #expect(try await !launchHasDiff(in: repo))

            // The reset preview was opened on private Off. A later hand edit wins.
            let external = "# hand edit\ncontext: {diff: true}\n"
            try external.write(to: mine, atomically: true, encoding: .utf8)
            await model.apply(.reset, to: privateOff, notices: notices, manager: nil)
            #expect(model.editError?.contains("refresh") == true)
            #expect(try String(contentsOf: mine, encoding: .utf8) == external)
            try picker.select(value: ConfigurationDestination.personalRepository)
            try await awaitReading { model.file?.declared == .bool(true) }
            watch.cancel()
            await watch.value
            model.refresh()
            watch = Task { await model.observe(repo.path) }
            try await awaitReading { model.editError == nil && model.file?.declared == .bool(true) }
            await model.apply(.reset, to: try #require(model.file), notices: notices, manager: nil)
            try await awaitReading { model.file?.diff.isEmpty == true }

            _ = try await lf.run(["config", "set", "context.diff", "false", "--shared"], cwd: linked)
            _ = try await lf.run(["-c", "user.name=Fixture", "-c", "user.email=fixture@example.org", "commit", "-am", "Set linked Diff"], cwd: linked, executable: "/usr/bin/git")
            #expect(try await !launchHasDiff(in: linked))
            let child = try await cliScope(linked)
            #expect(child.contextDiff.value == .bool(false))
            #expect(try await cliScope(repo).contextDiff.value == .bool(true))
            #expect(child.files.first { $0.owner == .personalRepository }?.path == privateOff.path)
            let linkedRoot = try await lf.run(["rev-parse", "--show-toplevel"], cwd: linked, executable: "/usr/bin/git")
                .trimmingCharacters(in: .whitespacesAndNewlines)
            #expect(child.files.first { $0.owner == .sharedRepository }?.path == linkedRoot + "/.lf/config.yaml")
            let get = try await lf.run(["config", "get", "context.diff", "--shared", "--json"], cwd: linked)
            #expect(try JSONDecoder().decode(ConfigurationFileValue.self, from: Data(get.utf8)).value == .bool(false))
            _ = try await lf.run(["config", "unset", "context.diff", "--shared"], cwd: linked)
            #expect(try String(contentsOf: linked.appending(path: ".lf/config.yaml"), encoding: .utf8) == "context: [README.md] # included at launch\n")
        } catch {
            watch.cancel()
            await watch.value
            await server.stop()
            throw error
        }
        watch.cancel()
        await watch.value
        await server.stop()
    }
}
#endif
