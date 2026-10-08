import Foundation
import Loopflow

private enum SessionFixtureKind: String {
    case flow
    case interactive

    var id: String { "ui-\(rawValue)" }

    var record: String {
        let state = self == .interactive ? "active" : "unknown"
        let attention = self == .interactive ? "null" : #""waiting""#
        let summary = self == .interactive ? "null" : #""Ready for review""#
        let work = self == .flow
            ? #"{"kind":"task","id":"task_00000000000000000000000000000001"}"#
            : "null"
        let membership = self == .flow
            ? #"{"kind":"step","flow":"task-design","flow_process_lfid":"fixture","step":"review-design","node":1,"iterations":[[]],"occurrence":"current"}"#
            : #"{"kind":"independent"}"#
        let actions: String
        switch self {
        case .interactive: actions = #"[{"kind":"open","label":"Open here","help":"Open this Session in a terminal","unavailable_reason":"This Session is active in another terminal; use Move here to transfer it"},{"kind":"move_here","label":"Move here","help":"Stop the other client and resume here; unsent text there is lost","unavailable_reason":null}]"#
        case .flow: actions = #"[{"kind":"open","label":"Open here","help":"Open this Session in a terminal","unavailable_reason":null}]"#
        }
        return """
        {
          "id": "\(id)", "interactive": true,
          "work": \(work),
          "title": "\(rawValue.capitalized) fixture",
          "detail": "fixture-provider",
          "cwd": "/tmp",
          "state": "\(state)", "attention": \(attention), "primary_scope": null, "task_primary": false,
          "ready_summary": \(summary),
          "work_path": null,
          "actions": \(actions),
          "title_source": "generated", "flow_membership": \(membership), "task_ids": [], "terminal_ids": [],
          "open_argv": ["/usr/bin/tail", "-f", "/dev/null"]
        }
        """
    }
}

private actor SessionFixtureStore {
    private let kind: SessionFixtureKind

    init(kind: SessionFixtureKind) {
        self.kind = kind
    }

    func run(_ args: [String]) throws -> String {
        if args == ["session", "list", "--json", "--page", "--interactive", "all", "--limit", "100"] {
            return #"{"entries":\#("[\(kind.record)]"),"next":null}"#
        }
        if args.starts(with: ["session", "connect", kind.id]), args.contains("--json") {
            return kind.record
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
