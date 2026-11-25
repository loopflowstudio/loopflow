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
        TaskGraph(name: "code", breakpoints: ["after-ship-review", "after-verify"], position: position,
                  steps: Self.code, currentStep: current, sentence: sentence)
    }

    private func run(running: Bool) -> TaskGraph.Position {
        .run(step: "plan", at: "implement", lfProcessId: "p", running: running)
    }

    @Test("Not started: every step ahead, setup's reason stands in for the sentence")
    func start() throws {
        let start = graph(.at(step: "start"), current: nil, sentence: "Not started.")
        let line = WorkflowLine(graph: start, auto: false)
        #expect(line.tones == Array(repeating: .ahead, count: 5))
        #expect(line.sentence == "Not started.")
        let waiting = WorkflowLine(graph: start, auto: false, setup: "Waiting for setup to finish")
        #expect(waiting.sentence == "Waiting for setup to finish")
    }

    @Test("Running: steps before are done, the current one runs, Rust's sentence is shown")
    func running() throws {
        let sentence = "build is running implement. You are next needed at ship-review."
        let running = graph(run(running: true), current: 1, sentence: sentence)
        let line = WorkflowLine(graph: running, auto: false)
        #expect(line.tones == [.done, .running, .ahead, .ahead, .ahead])
        #expect(line.sentence == sentence)
        // Setup's reason only stands in before the run starts.
        #expect(WorkflowLine(graph: running, auto: false, setup: "setup").sentence == sentence)
        #expect(try line.inspect().find(text: sentence).string() == sentence)
    }

    @Test("A dashboard tile's line is the sentence alone")
    func sentenceOnly() throws {
        let sentence = "build is running implement. You are next needed at ship-review."
        let line = WorkflowLine(graph: graph(run(running: true), current: 1, sentence: sentence), auto: false,
                                sentenceOnly: true)
        #expect(try line.inspect().find(viewWithAccessibilityIdentifier: "task-workflow-bar-live").text().string() == sentence)
        #expect(throws: (any Error).self) { try line.inspect().find(viewWithAccessibilityIdentifier: "task-workflow-bar-name") }
        #expect(throws: (any Error).self) { try line.inspect().find(viewWithAccessibilityIdentifier: "task-workflow-step-0") }
    }

    @Test("Stopped: the step it stopped in waits")
    func stopped() {
        let line = WorkflowLine(graph: graph(run(running: false), current: 1,
                                             sentence: "Stopped at build. `lf task run` starts it again."), auto: false)
        #expect(line.tones == [.done, .waiting, .ahead, .ahead, .ahead])
        #expect(line.sentence == "Stopped at build. `lf task run` starts it again.")
    }

    @Test("Standing at a breakpoint: the breakpoint waits")
    func waitingAtYou() {
        let line = WorkflowLine(graph: graph(.breakpoint(step: "after-ship-review", arrival: "arr_1"), current: 2,
                                             sentence: "At breakpoint after-ship-review."), auto: false)
        #expect(line.tones == [.done, .done, .waiting, .ahead, .ahead])
        #expect(line.sentence == "At breakpoint after-ship-review.")
    }

    @Test("Auto on: the sentence names auto, and the step still runs")
    func autoOn() {
        let sentence = "build is running implement. Auto stands in next at after-ship-review."
        let line = WorkflowLine(graph: graph(run(running: true), current: 1, sentence: sentence), auto: true)
        #expect(line.tones == [.done, .running, .ahead, .ahead, .ahead])
        #expect(line.sentence == sentence)
    }

    @Test("An outline: every step ahead, where you take part under the steps; a Task's bar shows the name and no sentence")
    func outline() throws {
        let outline = WorkflowOutline(steps: Self.code, breakpoints: ["after-ship-review", "after-verify"],
                                      sentence: "You take part at after-ship-review and after-verify.")
        let sheet = WorkflowLine(outline: outline)
        #expect(sheet.tones == Array(repeating: .ahead, count: 5))
        _ = try sheet.inspect().find(text: "after-ship-review")
        _ = try sheet.inspect().find(text: "You take part at after-ship-review and after-verify.")
        #expect(throws: (any Error).self) { try sheet.inspect().find(viewWithAccessibilityIdentifier: "task-workflow-bar-name") }
        let bar = WorkflowLine(name: "code", outline: outline)
        #expect(try bar.inspect().find(viewWithAccessibilityIdentifier: "task-workflow-bar-name").text().string() == "code")
        _ = try bar.inspect().find(text: "deliver")
        #expect(bar.sentence == nil)
    }

    @Test("At its end: every step is done")
    func end() {
        let line = WorkflowLine(graph: graph(.at(step: "end"), current: nil, sentence: "At its end."), auto: false)
        #expect(line.tones == Array(repeating: .done, count: 5))
        #expect(line.sentence == "At its end.")
    }

    @Test("The step that moves a Task is named from where its Graph stands")
    func runAction() {
        func action(_ position: TaskGraph.Position) -> TaskActions.Run? {
            graph(position, current: nil, sentence: "").runAction(issue: "LOO-5")
        }
        #expect(action(.at(step: "start")) == .init(label: "Start", help: "lf task run LOO-5"))
        #expect(action(.at(step: "ship-review")) == .init(label: "Run", help: "lf task run LOO-5: continue after ship-review"))
        #expect(action(run(running: false)) == .init(label: "Run", help: "lf task run LOO-5: run again at implement"))
        // A run carries the Task, or it is finished: nothing to press.
        #expect(action(run(running: true)) == nil)
        #expect(action(.at(step: "end")) == nil)
    }

    @Test("Alone, the sentence sets no line limit, so a phone shows all of it")
    func sentenceWraps() throws {
        let source = try String(contentsOf: URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().appendingPathComponent("Loopflow/Design/WorkflowLine.swift"), encoding: .utf8)
        #expect(!source.contains("lineLimit"))
    }
}
#endif
