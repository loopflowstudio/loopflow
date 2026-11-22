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
        .init(name: "verify", interactive: true),
    ]

    private func graph(_ position: TaskGraph.Position, current: Int?, sentence: String) -> TaskGraph {
        TaskGraph(name: "code", interactiveSteps: [], position: position,
                  steps: Self.code, currentStep: current, sentence: sentence)
    }

    private func run(running: Bool) -> TaskGraph.Position {
        .run(step: "plan", at: "implement", lfProcessId: "p", running: running)
    }

    @Test("Not started: every step ahead, setup's reason stands in for the sentence")
    func start() throws {
        let line = WorkflowLine(graph: graph(.at(step: "start"), current: nil, sentence: "Not started."), auto: false)
        #expect(WorkflowLine.tones(line.graph) == Array(repeating: .ahead, count: 5))
        #expect(line.sentence == "Not started.")
        let waiting = WorkflowLine(graph: line.graph, auto: false, setup: "Waiting for setup to finish")
        #expect(waiting.sentence == "Waiting for setup to finish")
    }

    @Test("Running: steps before are done, the current one runs, Rust's sentence is shown")
    func running() throws {
        let sentence = "build is running implement. You are next needed at ship-review."
        let line = WorkflowLine(graph: graph(run(running: true), current: 1, sentence: sentence), auto: false)
        #expect(WorkflowLine.tones(line.graph) == [.done, .running, .ahead, .ahead, .ahead])
        #expect(line.sentence == sentence)
        // Setup's reason only stands in before the run starts.
        #expect(WorkflowLine(graph: line.graph, auto: false, setup: "setup").sentence == sentence)
        #expect(try line.inspect().find(text: sentence).string() == sentence)
    }

    @Test("Stopped: the step it stopped in waits")
    func stopped() {
        let line = WorkflowLine(graph: graph(run(running: false), current: 1,
                                             sentence: "Stopped at build. `lf task run` starts it again."), auto: false)
        #expect(WorkflowLine.tones(line.graph) == [.done, .waiting, .ahead, .ahead, .ahead])
        #expect(line.sentence == "Stopped at build. `lf task run` starts it again.")
    }

    @Test("Waiting at you: the interactive step waits")
    func waitingAtYou() {
        let line = WorkflowLine(graph: graph(.at(step: "ship-review"), current: 2,
                                             sentence: "Waiting for you at ship-review."), auto: false)
        #expect(WorkflowLine.tones(line.graph) == [.done, .done, .waiting, .ahead, .ahead])
        #expect(line.sentence == "Waiting for you at ship-review.")
    }

    @Test("Auto on: the sentence names auto, and the step still runs")
    func autoOn() {
        let sentence = "build is running implement. Auto stands in next at ship-review."
        let line = WorkflowLine(graph: graph(run(running: true), current: 1, sentence: sentence), auto: true)
        #expect(WorkflowLine.tones(line.graph) == [.done, .running, .ahead, .ahead, .ahead])
        #expect(line.sentence == sentence)
    }

    @Test("At its end: every step is done")
    func end() {
        let line = WorkflowLine(graph: graph(.at(step: "end"), current: nil, sentence: "At its end."), auto: false)
        #expect(WorkflowLine.tones(line.graph) == Array(repeating: .done, count: 5))
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
