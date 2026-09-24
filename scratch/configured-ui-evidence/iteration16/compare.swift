import Foundation

// Compare captured Rust output directly with production Swift values. No fixtures,
// independent planning records, provider operations, or UI-trial credit.
@main
struct CompareLiveContract {
    static func main() throws {
        let directory = URL(fileURLWithPath: CommandLine.arguments[1])
        let roadmap = try JSONSerialization.jsonObject(with: Data(contentsOf: directory.appendingPathComponent("roadmap.json"))) as! [String: Any]
        let decoder = JSONDecoder()
        var ledger: [[String: Any]] = []
        var tasksByWork: [String: (String, String)] = [:]
        var conditions = Set<String>()
        var krCount = 0
        func check(_ actual: Any?, _ expected: Any?, _ field: String) {
            let lhs = actual ?? NSNull(), rhs = expected ?? NSNull()
            precondition((lhs as AnyObject).isEqual(rhs), "Contract mismatch: \(field)")
        }
        for wave in roadmap["waves"] as! [[String: Any]] {
            let identity = wave["wave"] as! [String: Any]
            guard identity["repo"] as? String == "/Users/jack/src/loopflow" else { continue }
            let rawEvidence = wave["projects"] as! [String: Any]
            let evidence = try decoder.decode(WorkEvidence<RoadmapProject>.self, from: JSONSerialization.data(withJSONObject: rawEvidence))
            guard case let .available(projects, truncated) = evidence else {
                preconditionFailure("Selected repository planning unavailable")
            }
            precondition(!truncated)
            let rawProjects = rawEvidence["items"] as! [[String: Any]]
            check(projects.count, rawProjects.count, "Project count")
            for (project, rawProject) in zip(projects, rawProjects) {
                let planning = rawProject["project"] as! [String: Any]
                for (key, actual) in ["id": project.id, "slug": project.project.slug, "name": project.project.name, "summary": project.project.summary, "definition": project.project.definition] {
                    check(actual, planning[key], "Project \(project.id).\(key)")
                }
                let krs = planning["krs"] as! [[String: Any]]
                check(project.project.krs.count, krs.count, "KR count")
                for (kr, raw) in zip(project.project.krs, krs) {
                    check(kr.text, raw["text"], "KR text")
                    check(kr.holds, raw["holds"], "KR proof")
                    krCount += 1
                }
                check(project.runtime?.workId, (rawProject["runtime"] as? [String: Any])?["work_id"], "Project Work")
                ledger.append(["kind": "project", "id": project.id, "name": project.project.name, "krs": krs.count, "result": "pass"])
                let rawTasks = rawProject["tasks"] as! [[String: Any]]
                check(project.tasks.count, rawTasks.count, "Task count")
                for (task, rawTask) in zip(project.tasks, rawTasks) {
                    let raw = rawTask["task"] as! [String: Any]
                    for (key, actual) in ["id": task.id, "identifier": task.task.identifier, "name": task.task.name, "description": task.task.description] {
                        check(actual, raw[key], "Task \(task.id).\(key)")
                    }
                    check(task.task.completed, raw["completed"], "Task completion")
                    let runtime = rawTask["runtime"] as? [String: Any]
                    check(task.runtime?.workId, runtime?["work_id"], "Task Work")
                    check(task.runtime?.projectId, runtime?["project_id"], "Task Project Work")
                    let condition = rawTask["condition"] as! [String: Any]
                    check(task.condition.state.rawValue, condition["state"], "Task condition")
                    check(task.condition.reason, condition["reason"], "Task condition reason")
                    check(task.condition.observedAt, condition["observed_at"], "Task condition time")
                    check(task.condition.evidenceAgeSeconds, condition["evidence_age_secs"], "Task evidence age")
                    let actions = rawTask["actions"] as! [String: Any]
                    check(task.actions.recommended?.rawValue, actions["recommended"], "Task action")
                    check(task.actions.reason, actions["reason"], "Task action reason")
                    let reference = rawTask["reference"] as! [String: Any]
                    let workspace = reference["workspace"] as? [String: Any]
                    check(task.reference.workspace?.worktree, workspace?["worktree"], "Task checkout")
                    check(task.reference.workspace?.localExists, workspace?["local_exists"], "Task checkout existence")
                    conditions.insert(task.condition.state.rawValue)
                    if let work = task.runtime?.workId {
                        tasksByWork[work] = (task.id, "\(identity["name"] as! String) / \(project.project.name) / \(task.task.identifier)")
                    }
                    ledger.append(["kind": "task", "id": task.id, "identifier": task.task.identifier, "condition": task.condition.state.rawValue, "result": "pass"])
                }
            }
        }
        func withoutNulls(_ value: Any) -> Any {
            if let dictionary = value as? [String: Any] { return dictionary.filter { !($0.value is NSNull) }.mapValues(withoutNulls) }
            if let array = value as? [Any] { return array.map(withoutNulls) }
            return value
        }
        let sessionData = try Data(contentsOf: directory.appendingPathComponent("sessions.json"))
        let sessions = try decoder.decode([SessionRecord].self, from: sessionData)
        let encoded = try JSONSerialization.jsonObject(with: JSONEncoder().encode(sessions))
        let rawSessions = try JSONSerialization.jsonObject(with: sessionData)
        check(withoutNulls(encoded), withoutNulls(rawSessions), "Every Session wire field")
        var joined = 0
        for session in sessions {
            if let work = session.work, work.kind == .task, let (taskID, path) = tasksByWork[work.id] {
                check(session.workPath, path, "Session exact Task ancestry")
                ledger.append(["kind": "session", "id": session.id, "task_id": taskID, "work_path": path, "state": session.state.rawValue, "result": "pass"])
                joined += 1
            } else {
                ledger.append(["kind": "session", "id": session.id, "state": session.state.rawValue, "result": "pass", "association": "unmatched; preserved"])
            }
        }
        let report: [String: Any] = ["rows": ledger, "records": ledger.count, "krs": krCount, "conditions": conditions.sorted(), "session_kinds": Set(sessions.map { $0.kind.rawValue }).sorted(), "sessions_joined_to_task": joined, "configured_ui_trials": 0]
        try JSONSerialization.data(withJSONObject: report, options: [.prettyPrinted, .sortedKeys]).write(to: directory.appendingPathComponent("comparison.json"))
        print("PASS: \(ledger.count) live Project/Task/Session records; \(krCount) KRs; \(joined) exact Session-to-Task joins. Conditions: \(conditions.sorted()). UI trials: zero.")
    }
}
