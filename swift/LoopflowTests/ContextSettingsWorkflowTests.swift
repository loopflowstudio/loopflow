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
            return prompt.contains("diff --git a/README.md b/README.md")
        }
        func cliScope(_ directory: URL) async throws -> ConfigurationScope {
            let text = try await lf.run(["config", "scope", "--json"], cwd: directory)
            return try JSONDecoder().decode(ConfigurationScope.self, from: Data(text.utf8))
        }

        // Desktop opens an installed Machine. Finish first-open migrations before connecting.
        _ = try await lf.run(["desktop", "portfolio", "--json"])
        let server = LocalServerFixture(lf)
        let model = ScopedSettingsModel(location: server.location())
        let notices = NoticeCenter()
        model.select(checkout: repo.path)
        var watch = Task { await model.observe() }
        func awaitReading(_ condition: () -> Bool, sourceLocation: SourceLocation = #_sourceLocation) async throws {
            let deadline = ContinuousClock.now + .seconds(10)
            while model.saving || !condition() {
                try #require(model.reading.failure == nil, "\(model.reading.failure ?? "")", sourceLocation: sourceLocation)
                try #require(ContinuousClock.now < deadline, "Context subscription last showed \(String(describing: model.file?.slots["context.diff"]?.declared.value)); edit error: \(model.errors.values.first?.message ?? "none")", sourceLocation: sourceLocation)
                try await Task.sleep(for: .milliseconds(20))
            }
        }

        do {
            _ = try await server.status()
            try await awaitReading { model.file != nil }
            #expect(model.file?.slots["context.diff"]?.entries.isEmpty == true)
            let view = ScopedSettings(model: model, pane: .context, checkouts: [repo.path], waves: [], notices: notices)
            let toggle = try view.inspect().find(ViewType.Toggle.self)
            #expect(try toggle.isOn())
            #expect(try await launchHasDiff(in: repo))
            try toggle.tap()
            try await awaitReading { model.file?.slots["context.diff"]?.declared.value == .bool(false) }
            #expect(try await !launchHasDiff(in: repo))

            let picker = try view.inspect().find(ViewType.Picker.self)
            try picker.select(value: ConfigurationDestination.sharedRepository)
            #expect(try view.inspect().find(ViewType.Toggle.self).isOn())
            var navigation = SettingsNavigation(search: " Diff ")
            let window = SettingsWindow(navigation: Binding(get: { navigation }, set: { navigation = $0 }),
                                        appearance: AppearanceStore(paths: .temporary(), systemDark: false),
                                        checkout: repo.path, portfolio: PortfolioService(), scope: model)
            func diffResult() throws -> InspectableView<ViewType.ClassifiedView> {
                try window.inspect().find(ViewType.NavigationSplitView.self).sidebarView().find(viewWithId: "context.diff")
            }
            // My Off wins, but the search switch edits Shared On.
            _ = try diffResult().find(text: "Shared repository settings · checkout")
            #expect(try diffResult().find(ViewType.Toggle.self).isOn())
            let sharedFile = try #require(model.file)
            #expect(try await model.preview(.reset, key: "context.diff", file: sharedFile) == "- diff: true")
            try diffResult().find(ViewType.Toggle.self).tap()
            #expect(navigation.search == " Diff ")
            try await awaitReading { model.file?.slots["context.diff"]?.declared.value == .bool(false) }
            #expect(try String(contentsOf: shared, encoding: .utf8) == original.replacingOccurrences(of: "true", with: "false"))
            try diffResult().find(ViewType.Button.self).tap()
            #expect(navigation.search.isEmpty)
            #expect(navigation.pane == .context)
            let detail = try window.inspect().find(ViewType.NavigationSplitView.self).detailView()
            let revealed = try detail.find(viewWithId: "context.diff")
            #expect(try revealed.accessibilityTraits().contains(.isSelected))
            #expect(try !revealed.find(ViewType.Toggle.self).isOn())
            try detail.find(button: "Undo").tap()
            try await awaitReading { model.file?.slots["context.diff"]?.declared.value == .bool(true) }
            #expect(try #require(model.file).hasOverrides(in: .context, catalog: model.catalogue))
            #expect(model.reading.value?.source("context.diff") == ConfigurationDestination.personalRepository.title)
            #expect(try await !launchHasDiff(in: repo))

            try picker.select(value: ConfigurationDestination.personalRepository)
            for (query, key) in [("linear.team", "repo.tracker"), ("branch diff", "context.diff"),
                                 ("context.diff_files", "context.diff_files"), ("Starting preference", "default_agent")] {
                navigation.search = query
                #expect(navigation.matches(in: model.catalogue).contains { $0.id == key })
            }
            navigation.search = "nothing matches this"
            let empty = try window.inspect().find(ViewType.NavigationSplitView.self).sidebarView()
            _ = try empty.find(text: "No matching settings")
            #expect(empty.findAll(SettingsSearchRow.self).isEmpty)
            let privateOff = try #require(model.file)
            await model.apply(.reset, key: "context.diff", to: privateOff, notices: notices, manager: nil)
            try await awaitReading { model.file?.slots["context.diff"]?.entries.isEmpty == true }
            #expect(try await launchHasDiff(in: repo))
            let undo = try #require(notices.current?.action)
            try picker.select(value: ConfigurationDestination.sharedRepository)
            undo.perform()
            try await awaitReading { model.reading.value?.settings["context.diff"]?.value == .bool(false) }
            #expect(try await !launchHasDiff(in: repo))

            // The reset preview was opened on private Off. A later hand edit wins.
            let external = "# hand edit\ncontext: {diff: true}\n"
            try external.write(to: mine, atomically: true, encoding: .utf8)
            await model.apply(.reset, key: "context.diff", to: privateOff, notices: notices, manager: nil)
            #expect(model.errors[.init(privateOff, "context.diff")]?.recovery == .refresh)
            #expect(try String(contentsOf: mine, encoding: .utf8) == external)
            try picker.select(value: ConfigurationDestination.personalRepository)
            try await awaitReading { model.file?.slots["context.diff"]?.declared.value == .bool(true) }
            navigation.search = "diff"
            #expect(try !diffResult().find(ViewType.Toggle.self).isDisabled())
            let staleRow = try view.inspect().find(viewWithId: "context.diff")
            _ = try staleRow.find(button: "Refresh")
            #expect(throws: (any Error).self) { try view.inspect().find(viewWithId: "context.paste").find(button: "Refresh") }
            try staleRow.find(button: "Refresh").tap()
            try await awaitReading { model.errors.isEmpty }
            await model.apply(.reset, key: "context.diff", to: try #require(model.file), notices: notices, manager: nil)
            try await awaitReading { model.file?.slots["context.diff"]?.entries.isEmpty == true }

            // A malformed file keeps the last reading visible until Refresh can read it.
            try Data("context: [".utf8).write(to: shared, options: .atomic)
            try await awaitReading { model.reading.failure != nil }
            #expect(try diffResult().find(ViewType.Toggle.self).isOn())
            #expect(try diffResult().find(ViewType.Toggle.self).isDisabled())
            watch.cancel()
            await watch.value
            try original.write(to: shared, atomically: true, encoding: .utf8)
            try window.inspect().find(ViewType.NavigationSplitView.self).sidebarView().find(button: "Refresh settings").tap()
            watch = Task { await model.observe() }
            let recoveryDeadline = ContinuousClock.now + .seconds(10)
            while model.reading.failure != nil {
                try #require(ContinuousClock.now < recoveryDeadline)
                try await Task.sleep(for: .milliseconds(20))
            }
            #expect(try !diffResult().find(ViewType.Toggle.self).isDisabled())

            _ = try await lf.run(["config", "set", "context.diff", "false", "--shared"], cwd: linked)
            _ = try await lf.run(["-c", "user.name=Fixture", "-c", "user.email=fixture@example.org", "commit", "-am", "Set linked Diff"], cwd: linked, executable: "/usr/bin/git")
            #expect(try await !launchHasDiff(in: linked))
            let child = try await cliScope(linked)
            #expect(child.settings["context.diff"]?.value == .bool(false))
            #expect(try await cliScope(repo).settings["context.diff"]?.value == .bool(true))
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
    @Test func scopedEditsAndPromoteRoundTripThroughTheCLI() async throws {
        let root = URL(fileURLWithPath: "/tmp/lf-scope-\(UUID().uuidString.prefix(8))").resolvingSymlinksInPath()
        let repo = root.appending(path: "project")
        let home = root.appending(path: "home")
        let fm = FileManager.default
        let waveID = "8ae21a68-8940-4553-a3ab-135133925e35"
        let wave = repo.appending(path: ".lf/wave/old")
        for dir in [home, wave, repo.appending(path: ".lf/skills"), repo.appending(path: ".lf/graphs")] {
            try fm.createDirectory(at: dir, withIntermediateDirectories: true)
        }
        defer { try? fm.removeItem(at: root) }
        try Data("---\nid: \(waveID)\nagent: codex\n---\nA Wave.\n".utf8).write(to: wave.appending(path: "GOAL.md"))
        try Data("- step: {name: probe, id: work}\n".utf8).write(to: repo.appending(path: ".lf/graphs/review.yaml"))
        try Data("Probe.\n".utf8).write(to: repo.appending(path: ".lf/skills/probe.md"))
        try Data("context: [README.md] # include\nrepo: {tracker: git}\n".utf8)
            .write(to: repo.appending(path: ".lf/config.yaml"))
        let lf = try BuiltLf(home: home, cwd: repo)
        _ = try await lf.run(["desktop", "portfolio", "--json"])
        let server = LocalServerFixture(lf)
        let model = ScopedSettingsModel(location: server.location())
        let notices = NoticeCenter()
        model.select(checkout: repo.path)
        func scope(_ arguments: [String] = [], target: [String] = []) async throws -> ConfigurationScope {
            let output = try await lf.run(arguments + ["config", "scope", "--json"] + target)
            return try JSONDecoder().decode(ConfigurationScope.self, from: Data(output.utf8))
        }
        func file(_ owner: ConfigurationDestination, _ arguments: [String] = []) async throws -> ConfigurationScope.File {
            let reading = try await scope(arguments)
            return try #require(reading.files.first { $0.owner == owner })
        }
        func set(_ key: String, _ value: String, _ owner: ConfigurationDestination) async throws {
            let address = try await file(owner)
            let saved = await model.apply(.set(value), key: key, to: address, notices: notices, manager: nil)
            try #require(saved, "\(model.errors.values.first?.message ?? "Save failed")")
        }
        func cliValue(_ key: String, _ arguments: [String] = []) async throws -> JSONValue {
            let output = try await lf.run(["config", "get", key, "--json"] + arguments)
            return try JSONDecoder().decode(ConfigurationFileValue.self, from: Data(output.utf8)).value
        }
        do {
            _ = try await server.status()
            #expect(try await file(.personalRepository).slots["context.docs"]?.declared == .absent)
            try await set("context.docs", "[]", .personalRepository)
            #expect(try await cliValue("context.docs") == .array([]))
            #expect(try await scope().settings["context.docs"]?.value == .array([]))
            let mine = try await file(.personalRepository)
            #expect(await model.apply(.reset, key: "context.docs", to: mine, notices: notices, manager: nil))
            #expect(try await cliValue("context.docs") == .null)
            #expect(try await scope().settings["context.docs"]?.value == .array([.string("README.md")]))

            try await set("agent", "claude", .sharedRepository)
            try await set("default_agent", "codex", .personalRepository)
            #expect(try await scope().settings["agent"]?.value == .string("claude"))
            #expect(try await cliValue("default_agent") == .string("codex"))
            try await set("repo.tracker", "{linear: {team: dev}}", .sharedRepository)
            #expect(try await cliValue("repo.tracker", ["--shared"]) == .object(["linear": .object(["team": .string("dev")])]))
            try await set("machine", "{session: local, tasks: local}", .personalRepository)
            try await set("machine", "{session: local, capacity: spare}", .personalRepository)
            #expect(try await cliValue("machine.tasks") == .null)
            #expect(try await cliValue("machine.capacity") == .string("spare"))

            try await set("diff_hidden", "[generated/**]", .personalRepository)
            let hidden = try #require(try await scope().settings["diff_hidden"])
            #expect(TaskFilesStore.hidden(from: hidden)?.contains(.init(pattern: "generated/**", source: nil)) == true)
            for (target, key) in [(["--skill", "probe"], "skill.probe.context.diff"),
                                  (["--graph", "review", "--step", "work"], "graphs.review.steps.work.context.diff")] {
                let reading = try await scope(target: target)
                let addressed = try #require(reading.files.first { $0.owner == .personalRepository })
                #expect(addressed.slots["context.diff"]?.key == key)
                #expect(await model.apply(.set("false"), key: "context.diff", to: addressed, notices: notices, manager: nil))
                #expect(try await scope(target: target).settings["context.diff"]?.value == .bool(false))
            }
            #expect(try await cliValue("context.diff") == .null)

            let source = try await file(.personalRepository)
            let review = try #require(await model.promote(.preview, key: "default_agent", file: source, notices: notices, manager: nil))
            #expect(review.changes.count == 2)
            #expect(try await cliValue("default_agent", ["--shared"]) == .null)
            _ = await model.promote(.confirm(review), key: "default_agent", file: source, notices: notices, manager: nil)
            #expect(try await cliValue("default_agent", ["--shared"]) == .string("codex"))
            #expect(try await cliValue("default_agent") == .null)
            model.select(checkout: repo.path, wave: waveID)
            try #require(notices.current?.action).perform()
            let deadline = ContinuousClock.now + .seconds(10)
            while try await cliValue("default_agent") == .null {
                try #require(ContinuousClock.now < deadline)
                try await Task.sleep(for: .milliseconds(20))
            }
            #expect(try await cliValue("default_agent", ["--shared"]) == .null)

            let sharedWave = try await file(.sharedWave, ["--wave", waveID])
            #expect(await model.apply(.set("true"), key: "chrome", to: sharedWave, notices: notices, manager: nil))
            let renamed = wave.deletingLastPathComponent().appending(path: "renamed")
            try fm.moveItem(at: wave, to: renamed)
            try fm.createDirectory(at: wave, withIntermediateDirectories: true)
            try Data("id: 7ae21a68-8940-4553-a3ab-135133925e35\n".utf8).write(to: wave.appending(path: "config.yaml"))
            let selected = try await scope(["--wave", waveID])
            #expect(selected.settings["agent"]?.value == .string("codex"))
            let selectedPath = try #require(selected.files.first { $0.owner == .sharedWave }?.path)
            let expectedURL = renamed.appending(path: "config.yaml")
            let selectedID = try #require(fm.attributesOfItem(atPath: selectedPath)[.systemFileNumber] as? NSNumber)
            let expectedID = try #require(fm.attributesOfItem(atPath: expectedURL.path)[.systemFileNumber] as? NSNumber)
            #expect(selectedID == expectedID)
        } catch {
            await server.stop()
            throw error
        }
        await server.stop()
    }
}
#endif
