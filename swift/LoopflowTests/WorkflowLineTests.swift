#if os(macOS)
import SwiftUI
import Testing
import ViewInspector
@testable import Loopflow
@testable import LoopflowMac

@MainActor
@Suite("Workflow line")
struct WorkflowLineTests {
    private static let code: [TaskGraph.Step] = [
        .init(name: "plan", interactive: false), .init(name: "build", interactive: false),
        .init(name: "ship-review", interactive: true), .init(name: "deliver", interactive: false),
        .init(name: "verify", interactive: false),
    ]

    private func graph(_ position: TaskGraph.Position, current: Int?, sentence: String) -> TaskGraph {
        TaskGraph(name: "code", control: .init(action: .run, label: "Run: implement"), interactiveSteps: [], position: position,
                  steps: Self.code, currentStep: current, sentence: sentence)
    }

    @Test("A Task's workflow line shows Rust's sentence alone, with setup standing in at start")
    func graphSentence() throws {
        let start = graph(.at(step: "start"), current: nil, sentence: "Not started.")
        #expect(WorkflowLine(graph: start).sentence == "Not started.")
        #expect(WorkflowLine(graph: start, setup: "Waiting for setup to finish").sentence == "Waiting for setup to finish")

        let sentence = "build is running implement. You are next needed at ship-review."
        let running = graph(.run(step: "plan", at: "implement", lfProcessId: "p", running: true), current: 1, sentence: sentence)
        let line = WorkflowLine(graph: running, setup: "setup")
        #expect(line.sentence == sentence)
        #expect(try line.inspect().find(viewWithAccessibilityIdentifier: "task-workflow-bar-live").text().string() == sentence)
        #expect(throws: (any Error).self) { try line.inspect().find(viewWithAccessibilityIdentifier: "task-workflow-step-0") }
    }

    @Test("An outline wraps its steps with where you take part underneath")
    func outline() throws {
        let outline = WorkflowOutline(start: .init(action: .run, label: "Run: plan"), steps: Self.code, interactiveSteps: [],
                                      sentence: "You take part at ship-review.")
        let sheet = WorkflowLine(outline: outline)
        _ = try sheet.inspect().find(text: "ship-review")
        _ = try sheet.inspect().find(text: "You take part at ship-review.")
        _ = try sheet.inspect().find(viewWithAccessibilityIdentifier: "task-workflow-step-0")
    }

    @Test("The sentence sets no line limit, so narrow surfaces wrap it")
    func sentenceWraps() throws {
        let source = try String(contentsOf: URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().appendingPathComponent("Loopflow/Design/WorkflowLine.swift"), encoding: .utf8)
        #expect(!source.contains("lineLimit"))
    }
}
#endif
