#if os(macOS)
import Foundation
import Testing
import ViewInspector
@testable import Loopflow
@testable import LoopflowMac

@Suite("Task directive editing", .serialized)
@MainActor
struct TaskDirectiveEditorTests {
    @Test("Directive updates keep their Task target and distinguish rejected writes from unavailable refreshed planning",
          arguments: [DirectiveSource.Outcome.rejected, .unreadable])
    fileprivate func updatesCapturedTask(outcome: DirectiveSource.Outcome) async throws {
        let source = try DirectiveSource()
        let model = PodiumModel(query: RegistryQuery { args, cwd in
            try await source.read(args, cwd: cwd)
        }, repoPath: "/src/loopflow")
        await model.refresh()
        model.select(.task(id: "issue-review"))
        let selected = try #require(model.task(id: "issue-review"))
        let original = selected.task.task.description
        #expect(throws: Never.self) { try WorkSurfaceView(model: model).inspect().find(button: "Edit description") }
        let editor = TaskDirectiveEditor(model: model, task: selected.task, wave: selected.wave.wave)
        #expect(try editor.inspect().find(ViewType.TextEditor.self).input() == original)
        let draft = "--Keep the human's wording\nIncluding `code`, $variables and \"quotes\"."
        // The editor captures its target; navigation cannot redirect the write.
        model.setRepoPath("/src/context")
        await source.setOutcome(outcome)
        do {
            try await model.updateTaskDirective(task: selected.task, wave: selected.wave.wave, text: draft)
            Issue.record("An unconfirmed update must surface an error")
        } catch {
            #expect(error.localizedDescription.contains(outcome == .rejected
                ? "Write rejected" : "Update accepted, but planning refresh failed"))
        }
        model.setRepoPath("/src/loopflow")
        #expect(model.task(id: "issue-review")?.task.task.description == original)
        if outcome == .unreadable { #expect(model.roadmap.errorMessage == "Planning offline") }

        await source.setOutcome(.success)
        try await model.updateTaskDirective(task: selected.task, wave: selected.wave.wave, text: draft)
        // Returned provider text deliberately differs from the submitted text.
        let authoritative = draft + "\nProvider-normalized."
        #expect(model.task(id: "issue-review")?.task.task.description == authoritative)
        #expect(model.task(id: "issue-review")?.task.task.completed == false)
        #expect(model.roadmap.errorMessage == nil)
        // Description renders as Markdown: inline code loses its backticks,
        // authored line breaks stay.
        let rendered = "--Keep the human's wording\nIncluding code, $variables and \"quotes\".\nProvider-normalized."
        #expect(throws: Never.self) { try WorkSurfaceView(model: model).inspect().find(text: rendered) }
    }

    @Test("Description blocks come from the Markdown parser without losing source text")
    func descriptionMarkdownBlocks() throws {
        let source = """
        # Outcome
        First line
        second line with [link](https://example.com)

        - outer **bold**
          - inner `code`
        1. one
        2. two

        > quoted

        ```swift
        let n = 1
        ```

        | A | B |
        | - | - |
        | 1 | 2 |
        """
        let blocks = MarkdownBlocks.blocks(source)
        let text = blocks.map { String($0.text.characters) }
        #expect(blocks.map(\.kind) == [.heading(1), .paragraph, .paragraph, .paragraph, .paragraph, .paragraph,
                                       .quote, .code, .row(header: true), .row(header: false)])
        #expect(text == ["Outcome", "First line\nsecond line with link", "outer bold", "inner code",
                         "one", "two", "quoted", "let n = 1", "A", "1"])
        #expect(blocks.map(\.marker) == [nil, nil, "•", "•", "1.", "2.", nil, nil, nil, nil])
        #expect(blocks.map(\.depth) == [0, 0, 1, 2, 1, 1, 0, 0, 0, 0])
        #expect(blocks[8].cells.map { String($0.characters) } == ["A", "B"])
        #expect(blocks[1].text.runs.contains { $0.link == URL(string: "https://example.com") })
        #expect(blocks[3].text.runs.contains { $0.inlinePresentationIntent == .code })
    }

    @Test("A polling response started before Save cannot restore the old directive")
    func oldReadCannotUndoSave() async throws {
        let source = try DirectiveSource()
        let model = PodiumModel(query: RegistryQuery { args, cwd in
            try await source.read(args, cwd: cwd)
        }, repoPath: "/src/loopflow")
        await model.refresh()
        let selected = try #require(model.task(id: "issue-review"))
        let gate = DirectiveReadGate()
        await source.holdNextRead(gate)
        let poll = Task { await model.refresh() }
        await gate.started()
        try await model.updateTaskDirective(task: selected.task, wave: selected.wave.wave, text: "New focus")
        await gate.release()
        await poll.value
        #expect(model.task(id: "issue-review")?.task.task.description == "New focus\nProvider-normalized.")
    }
}

private actor DirectiveSource {
    enum Outcome: Sendable { case success, rejected, unreadable }
    private var outcome = Outcome.success
    private var roadmap: String
    private let original: String
    private var written = false
    private var nextRead: DirectiveReadGate?

    init() throws {
        let fixture = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/dto/roadmap_snapshot.json")
        roadmap = try String(contentsOf: fixture, encoding: .utf8)
        original = roadmap
    }

    func setOutcome(_ value: Outcome) { outcome = value }
    func holdNextRead(_ gate: DirectiveReadGate) { nextRead = gate }

    func read(_ args: [String], cwd: String?) async throws -> String {
        switch args.first {
        case "pm":
            guard Array(args.prefix(7)) == ["pm", "task", "update", "--id", "issue-review", "--wave", "product"],
                  args.count == 8, args[7].hasPrefix("--notes="), cwd == "/src/loopflow" else {
                throw RegistryQueryError("Wrong Task update target")
            }
            if outcome == .rejected { throw RegistryQueryError("Write rejected") }
            let snapshot = try JSONDecoder().decode(RoadmapSnapshot.self, from: Data(original.utf8))
            let old = snapshot.waves[0].tasks.items.first { $0.id == "issue-review" }!.task.description
            let text = String(args[7].dropFirst("--notes=".count)) + "\nProvider-normalized."
            let oldJSON = String(decoding: try JSONEncoder().encode(old), as: UTF8.self)
            let newJSON = String(decoding: try JSONEncoder().encode(text), as: UTF8.self)
            roadmap = original.replacingOccurrences(of: oldJSON, with: newJSON)
            written = true
            return "product: updated task issue-review"
        case "roadmap":
            if written && outcome == .unreadable { throw RegistryQueryError("Planning offline") }
            let result = roadmap
            if let gate = nextRead {
                nextRead = nil
                await gate.wait()
            }
            return result
        case "ls", "session": return "[]"
        case "activity": return #"{"generated_at":1,"since":0,"limit":50,"truncated":false,"items":[]}"#
        default: throw RegistryQueryError("Unexpected directive proof operation")
        }
    }
}

private actor DirectiveReadGate {
    private var response: CheckedContinuation<Void, Never>?
    private var request: CheckedContinuation<Void, Never>?
    func wait() async {
        await withCheckedContinuation { continuation in
            response = continuation
            request?.resume()
            request = nil
        }
    }
    func started() async {
        if response != nil { return }
        await withCheckedContinuation { request = $0 }
    }
    func release() { response?.resume(); response = nil }
}
#endif
