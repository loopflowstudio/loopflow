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
        .init(name: "plan", breakpoint: false), .init(name: "build", breakpoint: false),
        .init(name: "after-ship-review", breakpoint: true), .init(name: "deliver", breakpoint: false),
        .init(name: "after-verify", breakpoint: true),
    ]

    private func graph(_ position: TaskGraph.Position, current: Int?, sentence: String) -> TaskGraph {
        TaskGraph(name: "code", control: .init(action: .run, label: "Run: plan"), breakpoints: ["after-ship-review", "after-verify"], position: position,
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

    @Test("Standalone breakpoints show a person icon and composed stages retain their names")
    func outline() throws {
        let outline = WorkflowOutline(start: .init(action: .run, label: "Run: plan"),
                                      steps: Self.code + [.init(name: "nested", breakpoint: true)],
                                      breakpoints: ["after-ship-review", "after-verify", "inside-nested"],
                                      sentence: "You take part after ship-review.")
        let sheet = try WorkflowLine(outline: outline).inspect()
        let stop = try sheet.find(viewWithAccessibilityIdentifier: "task-workflow-step-2")
        #expect(try stop.find(ViewType.Image.self).actualImage().name() == "person.fill")
        #expect(throws: (any Error).self) { try stop.find(text: "after-ship-review") }
        let nested = try sheet.find(viewWithAccessibilityIdentifier: "task-workflow-step-5")
        _ = try nested.find(text: "nested")
        #expect(try nested.find(ViewType.Image.self).actualImage().name() == "person.fill")
    }

    @Test("The sentence sets no line limit, so narrow surfaces wrap it")
    func sentenceWraps() throws {
        let source = try String(contentsOf: URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().appendingPathComponent("Loopflow/Design/WorkflowLine.swift"), encoding: .utf8)
        #expect(!source.contains("lineLimit"))
    }
}
#endif
