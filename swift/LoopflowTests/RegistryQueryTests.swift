// RegistryQuery decodes the `lf wave list/status/roadmap/usage --json` wire snapshots. The
// runner is injected, so these exercise parsing without spawning `lf`.

import Foundation
import Testing
@testable import Loopflow

@Suite("RegistryQuery")
struct RegistryQueryTests {
    @Test("Session adoption returns the served Task and preserves uncertain failure")
    func servedSessionAdoption() async throws {
        actor Adoption {
            var attempts = 0
            func submit(title: String) throws -> String {
                attempts += 1
                if title == "Uncertain" { throw RegistryQueryError("Outcome unknown") }
                return #"{"task":{"id":"task-retained","identifier":"FIX-1","title":"Keep the conversation"},"graph":null,"session_id":null,"milestone":"filed"}"#
            }
        }
        let adoption = Adoption()
        let query = RegistryQuery(write: { change in
            guard case .taskCreate(_, _, let title?, _, _, _, _, _, _) = change else {
                throw RegistryQueryError("Unexpected write")
            }
            return try await adoption.submit(title: title)
        }) { _, _ in throw RegistryQueryError("Unexpected execution launch") }
        let task = try await query.createTaskFromSession(id: "session-retained", title: "Keep the conversation", cwd: "/repo")
        #expect(task.id == "task-retained")
        #expect(task.title == "Keep the conversation")
        do {
            _ = try await query.createTaskFromSession(id: "session-retained", title: "Uncertain", cwd: "/repo")
            Issue.record("An uncertain adoption succeeded")
        } catch {
            #expect(error.localizedDescription == "Outcome unknown")
        }
        #expect(await adoption.attempts == 2)
    }

    @Test("Settings inspection retains skill and authored occurrence selectors")
    func settingsInspection() throws {
        let read = ServedRead.configSettings(directory: "/r", wave: nil, skill: nil, graph: "release", step: "write-code")
        #expect(read.argv == ["config", "settings", "--json", "--graph", "release", "--step", "write-code"])
        let data = try JSONEncoder().encode(read)
        let value = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
        #expect(value["kind"] as? String == "config.settings")
        #expect(value["graph"] as? String == "release")
        #expect(value["step"] as? String == "write-code")
    }

    @Test("A Graph log subscribes to the Task alone; the server reads it wherever the Task runs")
    func taskRoutedGraphLog() async throws {
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        let detail = try String(contentsOf: root.appendingPathComponent("tests/fixtures/dto/graph_detail.json"), encoding: .utf8)
        let expected = try JSONDecoder().decode(GraphProcessDetail.self, from: Data(detail.utf8))
        let overview = "{\"sessions\":[],\"graph_processes\":[],\"graph\":{\"name\":\"feature\",\"interactive_steps\":[],\"position\":{\"kind\":\"at\",\"step\":\"demo\"},\"steps\":[{\"name\":\"demo\",\"interactive\":true}],\"current_step\":0,\"sentence\":\"Waiting for you at demo.\"}}"
        let query = RegistryQuery(subscribeRead: { read in
            #expect(read == .taskGet(directory: "/", issue: "task"))
            let body = "{\"overview\":\(overview),\"graph_processes\":[\(detail)],\"comments\":{\"identifier\":\"task\",\"comments\":[],\"pending_sync\":[],\"conflicts\":{},\"refresh_error\":null},\"execution\":null,\"row\":null}"
            let lines = AsyncThrowingStream<Data, any Error> { continuation in
                continuation.yield(Data("{\"answers\":null,\"unavailable\":null,\"body\":\(body),\"read_at\":{\"planning\":7}}".utf8))
                continuation.finish()
            }
            return LineSubscription(lines: lines, checkpoint: { _ in }, cancel: {}, unreadable: { nil })
        }) { _, _ in throw RegistryQueryError("Unexpected one-shot read") }
        let subscription = try query.subscribeTask(task: "task")
        var iterator = subscription.lines.makeAsyncIterator()
        let line = try #require(await iterator.next())
        let frame = try WorkFrame.decode(part: .task,
            scope: WorkScope(repo: nil, headless: false, task: "task", activity: nil, taskListWave: nil), sequence: 0, line: line)
        #expect(frame.readAt == Revisions(["planning": 7]))
        guard case .task(let reading?) = frame.content else { Issue.record("Missing Task reading"); return }
        #expect(reading.graphProcesses == [expected])
        #expect(reading.work.graph?.position == .at(step: "demo"))
    }

    /// The JSON `read::tests::desktop_encodings_name_their_reads` decodes in Rust.
    @Test("Served reads encode as Rust's Read")
    func servedReadEncodings() throws {
        func object(_ value: some Encodable) throws -> NSDictionary {
            try #require(JSONSerialization.jsonObject(with: JSONEncoder().encode(value)) as? NSDictionary)
        }
        func object(_ json: String) throws -> NSDictionary {
            try #require(JSONSerialization.jsonObject(with: Data(json.utf8)) as? NSDictionary)
        }
        #expect(try object(ServedRead.sessionHistory(since: "7d", limit: 50, wave: nil, project: "p", task: nil))
            == object(#"{"kind":"session.history","since":"7d","limit":50,"wave":null,"project":"p","task":null}"#))
        let sessions = ServedRead.sessionList(directory: "/r", headless: true, after: nil, limit: nil, task: nil)
        #expect(try object(sessions) == object(#"{"kind":"session.list","directory":"/r","headless":true,"after":null,"limit":null,"task":null}"#))
        #expect(sessions.argv == ["session", "list", "--headless", "--json", "--limit", "0"])
        let page = ServedRead.sessionList(directory: "/r", headless: true, after: "cursor", limit: 100, task: nil)
        #expect(try object(page) == object(#"{"kind":"session.list","directory":"/r","headless":true,"after":"cursor","limit":100,"task":null}"#))
        #expect(page.argv == ["session", "list", "--json", "--page", "--headless", "--limit", "100", "--after", "cursor"])
        #expect(try object(ServedRead.taskDiff(issue: "FIX-1", path: nil, base: "parent", head: nil, draft: nil))
            == object(#"{"kind":"task.diff","issue":"FIX-1","path":null,"base":"parent","head":null,"draft":null}"#))
        let view = ServedRead.desktopViewGet(directory: "/r", expression: "state:backlog")
        #expect(try object(view) == object(#"{"kind":"desktop.view.get","directory":"/r","expression":"state:backlog"}"#))
        #expect(view.cwd == "/r")
        #expect(ServedRead.waveList.cwd == nil)
        #expect(try object(ServedRead.taskFilesWatch(issue: "FIX-1"))
            == object(#"{"kind":"task.files.watch","issue":"FIX-1"}"#))
        let receipt = ServedRead.creationGet(requestID: "reset", task: "LOO-1")
        #expect(try object(receipt) == object(#"{"kind":"creation.get","request_id":"reset","task":"LOO-1"}"#))
        #expect(receipt.argv == ["server", "receipt", "reset", "--task", "LOO-1", "--json"])
        #expect(try object(ServedRead.creationGet(requestID: "new", task: nil))
            == object(#"{"kind":"creation.get","request_id":"new","task":null}"#))
    }

    /// The JSON `write::tests::desktop_encodings_name_their_writes` decodes in Rust.
    @Test("Served writes encode as Rust's Write")
    func servedWriteEncodings() throws {
        func object(_ value: some Encodable) throws -> NSDictionary {
            try #require(JSONSerialization.jsonObject(with: JSONEncoder().encode(value)) as? NSDictionary)
        }
        func object(_ json: String) throws -> NSDictionary {
            try #require(JSONSerialization.jsonObject(with: Data(json.utf8)) as? NSDictionary)
        }
        let move = ServedWrite.taskMove(directory: "/r", issue: "FIX-1", wave: nil, machine: nil)
        #expect(try object(move) == object(#"{"kind":"task.move","directory":"/r","issue":"FIX-1","wave":null,"machine":null}"#))
        #expect(move.argv == ["task", "move", "FIX-1", "--no-wave", "--json"])
        #expect(move.cwd == "/r")
        let off = ServedWrite.taskAutoOff(directory: "/r", issue: "FIX-1")
        #expect(try object(off) == object(#"{"kind":"task.auto.off","directory":"/r","issue":"FIX-1","reason":null}"#))
        let keep = ServedWrite.repoTrackerAnswer(directory: "/r", action: "a1", answer: .keepTheirs)
        #expect(try object(keep) == object(#"{"kind":"repo.tracker.answer","directory":"/r","action":"a1","answer":"keep_theirs"}"#))
        #expect(keep.argv == ["repo", "tracker", "answer", "a1", "--keep-theirs", "--json"])
        #expect(off.argv == ["task", "auto", "off", "FIX-1", "--json"])
        let save = ServedWrite.taskSave(directory: "/", issue: "FIX-1", path: "a.txt", revision: "r", content: "text")
        #expect(try object(save) == object(
            #"{"kind":"task.save","directory":"/","issue":"FIX-1","path":"a.txt","revision":"r","content":"text"}"#))
        #expect(save.input == "text")
        let query = ServedWrite.desktopDashboardQuery(directory: "/r", id: "picked", expression: "-state:done", revision: nil)
        #expect(try object(query) == object(
            #"{"kind":"desktop.dashboard.query","directory":"/r","id":"picked","expression":"-state:done","revision":null}"#))
        #expect(query.argv == ["desktop", "dashboard", "query", "--", "picked", "-state:done"])
        let portfolio = ServedWrite.desktopPortfolioSet(change: ["remove", "/r"])
        #expect(try object(portfolio) == object(#"{"kind":"desktop.portfolio.set","change":{"action":"remove","path":"/r"}}"#))
        #expect(portfolio.argv == ["desktop", "portfolio", "remove", "/r", "--json"])
        #expect(portfolio.cwd == "/")
        let primary = ServedWrite.sessionResume(directory: "/r", id: nil, task: nil, wave: nil, agent: nil)
        #expect(try object(primary) == object(
            #"{"kind":"session.resume","directory":"/r","id":null,"task":null,"wave":null,"agent":null}"#))
        #expect(primary.argv == ["--repo", "/r", "session", "resume", "--prepare", "--json"])
        let task = ServedWrite.sessionResume(directory: "/r", id: nil, task: "FIX-1", wave: nil, agent: nil)
        #expect(try object(task) == object(
            #"{"kind":"session.resume","directory":"/r","id":null,"task":"FIX-1","wave":null,"agent":null}"#))
        #expect(task.argv == ["session", "resume", "--task", "FIX-1", "--prepare", "--json"])
        let restart = ServedWrite.sessionNew(directory: "/r", requestID: "fixture-thread", id: "session_1", task: "FIX-1", agent: "codex")
        #expect(try object(restart) == object(
            #"{"kind":"session.new","directory":"/r","request_id":"fixture-thread","id":"session_1","task":"FIX-1","agent":"codex"}"#))
        #expect(restart.argv == ["--agent", "codex", "session", "new", "session_1", "--request-id", "fixture-thread", "--prepare", "--json"])
    }

    @Test("Paired task and dashboard actions keep distinct wire kinds")
    func pairedWriteEncodings() throws {
        let writes: [(ServedWrite, String, [String], String)] = [
            (.taskClaim(directory: "/r", issue: "FIX-1"), "task.claim",
             ["task", "claim", "FIX-1", "--json"], #"{"directory":"/r","issue":"FIX-1"}"#),
            (.taskUnassign(directory: "/r", issue: "FIX-1"), "task.unassign",
             ["task", "unassign", "FIX-1", "--json"], #"{"directory":"/r","issue":"FIX-1"}"#),
            (.desktopDashboardPin(directory: "/r", id: "picked", revision: 3), "desktop.dashboard.pin",
             ["desktop", "dashboard", "pin", "--revision=3", "--", "picked"], #"{"directory":"/r","id":"picked","revision":3}"#),
            (.desktopDashboardUnpin(directory: "/r", id: "picked", revision: 3), "desktop.dashboard.unpin",
             ["desktop", "dashboard", "unpin", "--revision=3", "--", "picked"], #"{"directory":"/r","id":"picked","revision":3}"#),
            (.desktopDashboardHide(directory: "/r", id: "picked", member: "task:FIX-1"), "desktop.dashboard.hide",
             ["desktop", "dashboard", "hide", "query:picked/task:FIX-1"], #"{"directory":"/r","id":"picked","member":"task:FIX-1"}"#),
            (.desktopDashboardShow(directory: "/r", id: "picked", member: "task:FIX-1"), "desktop.dashboard.show",
             ["desktop", "dashboard", "show", "query:picked/task:FIX-1"], #"{"directory":"/r","id":"picked","member":"task:FIX-1"}"#),
        ]
        for (write, kind, argv, fields) in writes {
            var expected = try #require(JSONSerialization.jsonObject(with: Data(fields.utf8)) as? [String: Any])
            expected["kind"] = kind
            let encoded = try JSONSerialization.jsonObject(with: JSONEncoder().encode(write)) as? NSDictionary
            #expect(encoded == expected as NSDictionary)
            #expect(write.argv == argv)
            #expect(write.cwd == "/r")
        }
    }

    @Test("Session pages return a stable continuation and preserve renamed records")
    func sessionsReadPages() async throws {
        let row = try #require(JSONSerialization.jsonObject(with: Data(contentsOf: sessionFixtureURL())) as? [String: Any])
        var first = row
        first["id"] = "session-a"
        var last = row
        last["id"] = "session-z"
        last["title"] = "AAA renamed"
        let firstJSON = String(decoding: try JSONSerialization.data(withJSONObject:
            ["entries": [first], "next": "session-a"]), as: UTF8.self)
        let lastJSON = String(decoding: try JSONSerialization.data(withJSONObject:
            ["entries": [last], "next": NSNull()]), as: UTF8.self)
        let query = RegistryQuery { args, _ in
            args.contains("--after") ? lastJSON : firstJSON
        }
        let page = try await query.sessionPage(cwd: "/tmp/repo")
        let final = try await query.sessionPage(after: page.next, cwd: "/tmp/repo")
        #expect(page.entries.map(\.id) == ["session-a"])
        #expect(page.next == "session-a")
        #expect(final.entries.map(\.id) == ["session-z"])
        #expect(final.entries.first?.title == "AAA renamed")
        #expect(final.next == nil)
    }

    @Test("lf wave list can be decoded once for every repo")
    func allWavesDecode() async throws {
        let json = """
        [
          {
            "id": "goals",
            "name": "goals",
            "status": "ready",
            "goal": "ship the roadmap",
            "repo": "/tmp/repo-a",
            "active_tasks": 1,
            "created_at": null,
            "parent_wave_id": null,
            "machine": {
              "id": "home_00000000000000000000000000000001",
              "route": "local",
              "created_at": "1970-01-01T00:00:00Z",
              "observed_at": "1970-01-01T00:00:00Z"
            }
          },
          {
            "id": "other",
            "name": "other",
            "status": "ready",
            "goal": "g",
            "repo": "/tmp/repo-b",
            "active_tasks": 0,
            "created_at": null,
            "parent_wave_id": null,
            "machine": {
              "id": "home_00000000000000000000000000000001",
              "route": "local",
              "created_at": "1970-01-01T00:00:00Z",
              "observed_at": "1970-01-01T00:00:00Z"
            }
          }
        ]
        """
        let counter = CallCounter()
        let query = RegistryQuery { args, _ in
            await counter.increment()
            #expect(args == ["wave", "list", "--all", "--json"])
            return json
        }

        let waves = try await query.allWaves()
        #expect(await counter.value == 1)
        #expect(waves.map(\.id) == ["goals", "other"])
    }

    @Test("lf wave status maps the work hierarchy onto the wave")
    func statusMapsWork() async throws {
        let json = """
{
  "wave": {
    "id": "goals",
    "name": "goals",
    "status": "ready",
    "goal": "g",
    "repo": "/tmp/repo-a",
    "active_tasks": 1,
    "created_at": null,
    "parent_wave_id": null,
    "machine": {
      "id": "home_00000000000000000000000000000001",
      "route": "local",
      "created_at": "1970-01-01T00:00:00Z",
      "observed_at": "1970-01-01T00:00:00Z"
    }
  },
  "history": {
    "state": "ok",
    "truncated": false,
    "items": [
      {
        "repo": "/src/loopflow",
        "worktree": "/src/loopflow.task",
        "skill": "task/pursue",
        "usage": {
          "streams": 1,
          "final_streams": 1,
          "gaps": 0,
          "input_tokens": 1000,
          "output_tokens": 200,
          "total_input_tokens": 1000,
          "peak_input_tokens": 900,
          "context_window_tokens": 200000,
          "reasoning_tokens": null,
          "cache_read_tokens": 800,
          "cache_write_tokens": null,
          "cost_usd": 0.25
        },
        "evidence_gaps": 0,
        "harness": "claude",
        "model": "opus",
        "surface": "headless",
        "session_id": "run_00000000000000000000000000000001",
        "captured": 12,
    "artifact_key": "run_00000000000000000000000000000001",
        "caller_artifact_key": null,
        "observed_at": 100,
        "origin": null,
        "recorded_outcome": "completed",
    "recorded_at": 110,
        "providers": [],
        "task_id": "task_22222222222222222222222222222222",
        "wave_id": "11111111-1111-4111-8111-111111111111",
        "task_identifier": "INF-123",
        "wave_name": "goals"
      }
    ]
  },
  "project": {
    "state": "ok",
    "value": {
      "id": "project-1",
      "tracker_id": null,
      "url": null,
      "unbound": null,
      "sync": {"actions": []},
      "slug": "release-feedback",
      "name": "current",
      "status": "started"
    }
  },
  "tasks": {
    "state": "ok",
    "items": [
      {
        "task": {
          "repo_id": "repo_00000000000040008000000000000001",
          "repo": "/tmp/repo-a",
          "project_id": "project-1",
          "wave_id": "goals",
          "id": "issue-1",
          "identifier": "INF-123",
          "name": "Wire it",
          "description": "",
          "rank": 1,
          "state": "unstarted", "completed_at": null, "due_date": null, "tracker_state": {"state": "available", "synced_at": null}, "tracker_id": null, "parent_id": null, "sync": {"actions": []},
          "assignee": null
        },
        "reference": {
          "issue_url": "https://linear.app/loopflow/issue/INF-123/wire-it",
          "workspace": {
            "slug": "wire-it",
            "branch": "jack/inf-123", "base_commit": "fixture-base",
            "worktree": "/task-wt"
          }
        },
        "runtime": {
          "work_id": "issue-1",
          "status": "unstarted",
          "reason": "ready",
          "updated_at": "2026-07-06T00:00:00Z",
          "provider": "codex",
          "owner": {"person": "Jack Heart", "person_id": null, "machine": {"id": null, "label": null}}
        },
        "graph_name":"feature","graph_source":"repository","latest_graph_process":null,"execution":null,"lifecycle":"unstarted","selection_states":["unstarted","backlog"],"run_control":{"unavailable":null},
        "next_move": {
          "owner": "task",
          "reason": "ready"
        },
        "condition": {
          "state": "clear",
          "reason": "ready",
          "observed_at": "2026-07-06T00:01:00Z",
          "evidence_age_secs": 60,
          "local_progress": {
            "state": "observed",
            "unsettled": false,
            "dirty": false,
            "authored_commits": false,
            "recovery_required": false,
            "reason": null
          },
          "unresolved_execution": true
        },
        "actions": {
          "recommended": "run",
          "reason": "resume the parked Task"
        },
        "pr": null,
        "section": "working",
        "owned_by_you": false
      }
    ],
    "truncated": false
  }
}
"""
        let query = RegistryQuery { args, _ in
            #expect(args == ["wave", "plan", "goals", "--json"])
            return json
        }

        let result = try await query.status(wave: "goals", cwd: nil)
        #expect(result.wave.id == "goals")
        #expect(result.wave.goal == "g")



        #expect(result.workMap.tasks.items[0].task.identifier == "INF-123")
        #expect(result.workMap.tasks.items[0].reference.issueUrl?.absoluteString.contains("INF-123") == true)
        #expect(result.workMap.tasks.items[0].reference.workspace?.slug == "wire-it")
        #expect(result.workMap.tasks.items[0].reference.workspace?.worktree == "/task-wt")
        #expect(result.workMap.tasks.items[0].reference.workspace?.branch == "jack/inf-123")
        #expect(result.history.items[0].skill == "task/pursue")
        #expect(result.history.items[0].usage.inputTokens == 1000)
        #expect(result.workMap.tasks.items[0].condition.state == .clear)
        #expect(result.workMap.tasks.items[0].actions.recommended == .run)
    }

    @Test("lf wave show is one optionally scoped machine query")
    func roadmapUsesOneMachineQuery() async throws {
        let query = RegistryQuery { args, cwd in
            #expect(args == ["plan", "list", "product", "--json"])
            #expect(cwd == "/")
            return #"{"generated_at":"2026-07-15T00:00:00Z","waves":[],"tasks":{"state":"ok","items":[],"truncated":false}}"#
        }

        let result = try await query.roadmap(wave: "product")
        #expect(result.generatedAt == "2026-07-15T00:00:00Z")
        #expect(result.waves.isEmpty)
    }

    @Test("lf wave show requests every repository when no Wave narrows it")
    func roadmapRequestsAllRepositories() async throws {
        let query = RegistryQuery { args, cwd in
            #expect(args == ["plan", "list", "--all", "--json"])
            #expect(cwd == "/")
            return #"{"generated_at":"2026-07-15T00:00:00Z","waves":[],"tasks":{"state":"ok","items":[],"truncated":false}}"#
        }

        let result = try await query.roadmap()
        #expect(result.waves.isEmpty)
    }

    @Test("lf history composes Work filters before the bounded result")
    func workActivityUsesOneFilteredQuery() async throws {
        let json = try String(contentsOf: workActivityFixtureURL(), encoding: .utf8)
        let query = RegistryQuery { args, cwd in
            #expect(args == [
                "session", "history", "--since", "7d", "--limit", "50",
                "--wave", "product", "--project", "mac-surface-ux",
                "--task", "W2-144", "--json",
            ])
            #expect(cwd == nil)
            return json
        }

        let result = try await query.workActivity(
            wave: "product",
            project: "mac-surface-ux",
            task: "W2-144"
        )
        #expect(result.items[0].subject == "W2-144")
    }

    /// Unreadable evidence must reach the surface as its reason, never as an
    /// empty list — a broken ledger is not a quiet wave.
    @Test("lf status keeps unavailable evidence unavailable")
    func statusKeepsUnavailableEvidence() async throws {
        let json = """
        {
          "wave": {
            "id": "goals",
            "name": "goals",
            "status": "ready",
            "goal": "g",
            "repo": "/tmp/repo-a",
            "active_tasks": 0,
            "created_at": null,
            "parent_wave_id": null,
            "machine": {
              "id": "home_00000000000000000000000000000001",
              "route": "local",
              "created_at": "1970-01-01T00:00:00Z",
              "observed_at": "1970-01-01T00:00:00Z"
            }
          },
          "history": {
            "state": "unavailable",
            "reason": "Session history unavailable: disk is gone"
          },
          "project": {
            "state": "unavailable",
            "reason": "Project planning unavailable"
          },
          "tasks": {
            "state": "ok",
            "items": [],
            "truncated": false
          }
        }
        """
        let query = RegistryQuery { _, _ in json }

        let result = try await query.status(wave: "goals", cwd: nil)
        #expect(result.history.unavailableReason == "Session history unavailable: disk is gone")
        #expect(result.history.items.isEmpty)
    }

    @Test("Task workspace queries preserve paths and binary/truncation evidence, outside the checkout")
    func taskWorkspaceQueriesDecode() async throws {
        let query = RegistryQuery { args, cwd in
            #expect(cwd == nil)
            switch args {
            case ["task", "changes", "INF-123", "--base", "parent", "--json"]:
                return #"{"issue_identifier":"INF-123","task_id":"ts_1","base_commit":"abc","head_commit":"def","commits":[],"files":[{"path":"src/parser.rs","old_path":null,"committed":true,"staged":false,"unstaged":true,"untracked":false,"changes":2}],"scratch":[],"scratch_truncated":false}"#
            case ["task", "diff", "INF-123", "src/parser.rs", "--base", "parent", "--json"]:
                return #"{"issue_identifier":"INF-123","task_id":"ts_1","path":"src/parser.rs","base_commit":"abc","patch":"@@ -1 +1 @@","binary":false,"truncated":false}"#
            case ["task", "file", "INF-123", "src/parser.rs", "--json"]:
                return #"{"issue_identifier":"INF-123","task_id":"ts_1","path":"src/parser.rs","content":"fn parse() {}\n","state":"text","revision":"hash","recoveries": [], "size_bytes":14}"#
            default:
                throw RegistryQueryError("unexpected argv: \(args)")
            }
        }

        let changes = try await query.taskChanges(issue: "INF-123")
        #expect(changes.taskId == "ts_1")
        #expect(changes.files[0].path == "src/parser.rs")
        #expect(changes.files[0].committed && changes.files[0].unstaged)

        let diff = try await query.taskDiff(
            issue: "INF-123",
            path: "src/parser.rs"
        )
        #expect(diff.patch == "@@ -1 +1 @@")
        #expect(!diff.binary && !diff.truncated)

        let file = try await query.taskFile(
            issue: "INF-123",
            path: "src/parser.rs"
        )
        #expect(file.content == "fn parse() {}\n")
        #expect(file.sizeBytes == 14)
    }

    @Test("Sessions list and recover through the Session CLI")
    func sessionsUseCli() async throws {
        let sessionsJSON = try String(
            contentsOf: sessionsFixtureURL(),
            encoding: .utf8
        )
        let sessions = try JSONDecoder().decode(
            [SessionRecord].self,
            from: Data(sessionsJSON.utf8)
        )
        let sessionData = try Data(contentsOf: sessionFixtureURL())
        let session = try JSONDecoder().decode(SessionRecord.self, from: sessionData)
        let sessionJSON = String(
            data: try JSONEncoder().encode(session),
            encoding: .utf8
        )!
        let query = RegistryQuery { args, cwd in
            #expect(cwd == "/tmp/repo")
            switch args {
            case ["session", "list", "--json", "--page", "--limit", "100"]:
                return #"{"entries":\#(sessionsJSON),"next":null}"#
            case ["session", "resume", session.id, "--prepare", "--json"],
                 ["session", "resume", "--task", "INF-123", "--prepare", "--json"]:
                return sessionJSON
            default:
                throw RegistryQueryError("unexpected argv: \(args)")
            }
        }

        let listed = try await query.sessionPage(cwd: "/tmp/repo").entries
        let opened = try await query.prepareSession(id: session.id, cwd: "/tmp/repo")

        #expect(listed == sessions)
        #expect(opened == session)
        let primary = try await query.prepareTaskSession(issue: "INF-123", cwd: "/tmp/repo")
        #expect(primary == session)
    }

    @Test("Activity requires Session and input identity instead of unrelated process fallbacks")
    func invocationActivityIsNotSessionHistory() async throws {
        let json = #"{"truncated":false,"items":[{"id":"event","recorded_at":1784606300,"summary":"Input completed","work":{"kind":"task","id":"task-1"},"subject":"LOO-1","fact":{"kind":"input_completion_recorded","graph_lf_process_id":"invocation-1","trace_id":"trace-1","lf_process_id":"process-1","status":"ok"}}]}"#
        let query = RegistryQuery { _, _ in json }
        await #expect(throws: (any Error).self) { try await query.workActivity() }
    }

    @Test("lf usage preserves direct Run evidence through a Work drill")
    func usageDecodes() async throws {
        let json = """
[
  {
    "repo": "/src/loopflow",
    "worktree": null,
    "skill": "implement",
    "usage": {
      "streams": 2,
      "final_streams": 1,
      "gaps": 1,
      "input_tokens": 120,
      "output_tokens": null,
      "total_input_tokens": 120,
      "peak_input_tokens": 100,
      "context_window_tokens": 200000,
      "reasoning_tokens": null,
      "cache_read_tokens": 80,
      "cache_write_tokens": null,
      "cost_usd": 0.25
    },
    "evidence_gaps": 1,
    "harness": "codex",
    "model": "gpt",
    "surface": "headless",
    "session_id": "run_00000000000000000000000000000001",
    "captured": 12,
    "artifact_key": "run_00000000000000000000000000000001",
    "caller_artifact_key": null,
    "observed_at": 100,
    "origin": null,
    "recorded_outcome": "completed",
    "recorded_at": 110,
    "providers": [],
    "task_id": "task_22222222222222222222222222222222",
    "wave_id": null,
    "task_identifier": "LOO-265",
    "wave_name": null
  }
]
"""
        let query = RegistryQuery { args, _ in
            #expect(args == [
                "session", "usage", "--days", "7",
                "--wave", "product", "--project", "sessions", "--task", "LOO-265", "--json",
            ])
            return json
        }

        let runs = try await query.usage(
            days: 7,
            wave: "product",
            project: "sessions",
            task: "LOO-265"
        )
        #expect(runs[0].usage.inputTokens == 120)
        #expect(runs[0].usage.outputTokens == nil)
        #expect(runs[0].usage.finalStreams == 1)
        #expect(runs[0].evidenceGaps == 1)
    }

    @Test("lf doctor decodes every check")
    func doctorDecodes() async throws {
        let json = """
        {
          "checks": [
            {
              "name": "store",
              "status": "ok",
              "detail": "selected database is compatible with this build"
            }
          ]
        }
        """
        let query = RegistryQuery { args, _ in
            #expect(args == ["machine", "doctor", "--json"])
            return json
        }

        let report = try await query.doctor()
        #expect(report.checks[0].name == "store")
        #expect(report.checks[0].status == "ok")
    }

    @Test("a failed lf query surfaces as an error")
    func failedQueryThrows() async {
        let query = RegistryQuery { _, _ in throw RegistryQueryError("lf exploded") }
        await #expect(throws: RegistryQueryError.self) {
            _ = try await query.allWaves()
        }
    }
}

private func workActivityFixtureURL(sourceFile: String = #filePath) -> URL {
    URL(fileURLWithPath: sourceFile)
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .appendingPathComponent("tests/fixtures/dto/work_activity_snapshot.json")
}

private func sessionsFixtureURL(sourceFile: String = #filePath) -> URL {
    URL(fileURLWithPath: sourceFile)
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .appendingPathComponent("tests/fixtures/dto/sessions.json")
}

private func sessionFixtureURL(sourceFile: String = #filePath) -> URL {
    URL(fileURLWithPath: sourceFile)
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .appendingPathComponent("tests/fixtures/dto/session.json")
}

private actor CallCounter {
    private var count = 0

    var value: Int { count }

    func increment() {
        count += 1
    }
}
