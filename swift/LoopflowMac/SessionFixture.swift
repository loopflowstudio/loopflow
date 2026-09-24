import Foundation
import Loopflow

private enum SessionFixtureKind: String {
    case ask
    case flow
    case interactive

    var id: String { "ui-\(rawValue)" }

    var record: String {
        let state = self == .interactive ? "active" : "ready"
        let summary = self == .interactive ? "null" : #""Ready for review""#
        let work = self == .flow
            ? #"{"kind":"task","id":"task_00000000000000000000000000000001"}"#
            : "null"
        let actions: String
        switch self {
        case .interactive: actions = #"[{"kind":"open","label":"Open here","help":"Open this Session in a terminal","unavailable_reason":"This Session is active in another terminal; use Move here to transfer it"},{"kind":"move_here","label":"Move here","help":"Stop the other client and resume here; unsent text there is lost","unavailable_reason":null},{"kind":"complete","label":"Complete","help":"Stop the provider and remove this Session; native history remains resumable","unavailable_reason":null}]"#
        case .ask: actions = #"[{"kind":"open","label":"Open here","help":"Open this Session in a terminal","unavailable_reason":null},{"kind":"complete","label":"Complete","help":"Complete the conversation and resume its blocked caller","unavailable_reason":null}]"#
        case .flow: actions = #"[{"kind":"open","label":"Open here","help":"Open this Session in a terminal","unavailable_reason":null},{"kind":"approve","label":"Approve and continue","help":"Approve and continue the Task","unavailable_reason":null},{"kind":"iterate","label":"Iterate","help":"Return to autonomous Task work with your direction","unavailable_reason":null}]"#
        }
        return """
        {
          "id": "\(id)",
          "kind": "\(rawValue)",
          "work": \(work),
          "title": "\(rawValue.capitalized) fixture",
          "detail": "fixture-provider",
          "cwd": "/tmp",
          "state": "\(state)",
          "ready_summary": \(summary),
          "work_path": null,
          "actions": \(actions),
          "terminal_ids": [],
          "open_argv": ["/usr/bin/tail", "-f", "/dev/null"]
        }
        """
    }
}

private actor SessionFixtureStore {
    private let kind: SessionFixtureKind
    private var unresolved = true

    init(kind: SessionFixtureKind) {
        self.kind = kind
    }

    func run(_ args: [String]) throws -> String {
        if args == ["session", "list", "--json"] {
            return unresolved ? "[\(kind.record)]" : "[]"
        }
        if args.starts(with: ["session", "open", kind.id]), args.contains("--json") {
            guard unresolved else { throw RegistryQueryError("Session \(kind.id) was not found") }
            return kind.record
        }
        if args == ["session", "complete", kind.id] {
            guard kind != .flow else {
                throw RegistryQueryError("Task FlowStep Sessions cannot complete")
            }
            unresolved = false
            return "Session completed"
        }
        if args.count >= 4,
           args[0] == "session",
           ["approve", "iterate"].contains(args[1]),
           args[2] == kind.id {
            guard kind == .flow else {
                throw RegistryQueryError("Only Task FlowStep Sessions accept decisions")
            }
            unresolved = false
            return "Task FlowStep resolved"
        }
        if args == ["roadmap", "--all", "--json"] {
            return #"{"generated_at":"2026-08-30T00:00:00Z","waves":[]}"#
        }
        throw RegistryQueryError("Unsupported Session fixture command: \(args.joined(separator: " "))")
    }
}

enum SessionFixture {
    static let query: RegistryQuery? = {
        guard AppTestMode.current() == .sessionFixtures else { return nil }
        let raw = ProcessInfo.processInfo.environment["LOOPFLOW_UI_TEST_SESSION_KIND"]
        let kind = raw.flatMap(SessionFixtureKind.init(rawValue:)) ?? .interactive
        let store = SessionFixtureStore(kind: kind)
        return RegistryQuery { args, _ in
            try await store.run(args)
        }
    }()
}
