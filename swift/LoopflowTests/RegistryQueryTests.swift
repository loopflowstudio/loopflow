// RegistryQuery decodes the `lf wave list/status/roadmap/usage --json` wire snapshots. The
// runner is injected, so these exercise parsing without spawning `lf`.

import Foundation
import Testing
@testable import Loopflow

@Suite("RegistryQuery")
struct RegistryQueryTests {
    @Test("Graph logs read captured execution from the content Machine, never a peer path locally")
    func contentRoutedGraphLog() async throws {
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        let detail = try String(contentsOf: root.appendingPathComponent("tests/fixtures/dto/graph_detail.json"), encoding: .utf8)
        let expected = try JSONDecoder().decode(GraphProcessDetail.self, from: Data(detail.utf8))
        let entry = String(decoding: try JSONEncoder().encode(expected.entry), as: UTF8.self)
        let query = RegistryQuery { args, cwd in
            guard args.starts(with: ["--machine", "peer", "--repository", "plan"]), cwd == nil else {
                throw RegistryQueryError("A peer Graph log must never use local execution")
            }
            let command = Array(args.dropFirst(4))
            if command.starts(with: ["graph", "list"]) {
                return "{\"entries\":[\(entry)],\"next\":null}"
            }
            if command == ["graph", "show", expected.entry.id, "--processes", "--json"] { return detail }
            throw RegistryQueryError("Unexpected execution")
        }
        let graphs = try await query.onMachine("peer", repository: "plan").taskGraphProcesses(task: "task")
        #expect(graphs == [expected])
    }

    @Test("Recording a remote reading preserves its Machine and transport", arguments: [false, true])
    func recordingRemoteQuery(recordFirst: Bool) async throws {
        let json = #"{"id":"peer"}"#
        let local = RegistryQuery { args, cwd in
            guard args.starts(with: ["--machine", "peer", "--repository", "plan"]), cwd == nil else {
                throw RegistryQueryError("The reading lost its remote selectors")
            }
            return json
        }
        let record: @Sendable (String) -> Void = { #expect($0 == json) }
        let query = recordFirst
            ? local.recording(record).onMachine("peer", repository: "plan")
            : local.onMachine("peer", repository: "plan").recording(record)
        #expect(query.remoteMachine == "peer")
        #expect(try await query.localMachineId() == "peer")
        #expect(!query.streamsWork)
        #expect(local.remoteMachine == nil)
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
            #expect(args == ["wave", "list", "--all", "--current", "--json"])
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
  "metric_portfolio": {
    "metrics": [],
    "contract_issues": []
  },
  "project_readiness": {"state": "ready", "project_id": "project-1", "observed_at": null, "pending_successor": null, "activation": null},
  "projects": {
    "state": "ok",
    "items": [
      {
        "id": "project-1",
        "linear_id": null,
        "sync": {"connected": false, "changes": []},
        "slug": "release-feedback",
        "name": "current",
        "workflow": "task-design",
        "status": "started", "current": true,
        "metric_targets": [],
        "krs": [
          {
            "text": "Fast loops",
            "holds": false
          }
        ]
      }
    ],
    "truncated": false
  },
  "tasks": {
    "state": "ok",
    "items": [
      {
        "task": {
          "repository_id": "repo_00000000000040008000000000000001",
          "repo": "/tmp/repo-a",
          "project_id": "project-1",
          "wave_id": "goals",
          "id": "issue-1",
          "identifier": "INF-123",
          "name": "Wire it",
          "description": "",
          "rank": 1,
          "state": "unstarted", "completed_at": null, "due_date": null, "acquisition": {"state": "available", "observed_at": null}, "linear_id": null, "sync": {"connected": false, "changes": []},
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
          "owner": {"person": "Jack Heart", "machine": {"id": null, "label": null}}
        },
        "graph_name":"feature","graph_source":"wave","latest_graph_process":null,"execution":null,"run_control":{"unavailable":null},"ship_control":{"unavailable":null},"shipping":null,
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
          "recommended": "resume",
          "reason": "resume the parked Task"
        },
        "pr": null,
        "section": "now"
      }
    ],
    "truncated": false
  },
  "unavailable_tasks": []
}
"""
        let query = RegistryQuery { args, _ in
            #expect(args == ["desktop", "wave", "goals", "--json"])
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
        #expect(result.workMap.tasks.items[0].actions.recommended == .resume)
    }

    @Test("lf wave show is one optionally scoped machine query")
    func roadmapUsesOneMachineQuery() async throws {
        let query = RegistryQuery { args, cwd in
            #expect(args == ["desktop", "plan", "product", "--json"])
            #expect(cwd == nil)
            return #"{"generated_at":"2026-07-15T00:00:00Z","waves":[],"tasks":{"state":"ok","items":[],"truncated":false}}"#
        }

        let result = try await query.roadmap(wave: "product")
        #expect(result.generatedAt == "2026-07-15T00:00:00Z")
        #expect(result.waves.isEmpty)
    }

    @Test("lf wave show requests every repository when no Wave narrows it")
    func roadmapRequestsAllRepositories() async throws {
        let query = RegistryQuery { args, cwd in
            #expect(args == ["desktop", "plan", "--all", "--json"])
            #expect(cwd == nil)
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
                "history", "--since", "7d", "--limit", "50",
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
          "metric_portfolio": {
            "metrics": [],
            "contract_issues": []
          },
          "project_readiness": {"state": "unavailable", "project_id": null, "observed_at": null, "pending_successor": null, "activation": null},
          "projects": {
            "state": "unavailable",
            "reason": "Project planning unavailable"
          },
          "tasks": {
            "state": "ok",
            "items": [],
            "truncated": false
          },
          "unavailable_tasks": []
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
            case ["task", "diff", "INF-123", "--files", "--base", "parent", "--json"]:
                return #"{"issue_identifier":"INF-123","task_id":"ts_1","base_commit":"abc","head_commit":"def","files":[{"path":"src/parser.rs","old_path":null,"committed":true,"staged":false,"unstaged":true,"untracked":false}],"scratch":[],"scratch_truncated":false}"#
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
            case ["desktop", "sessions", "--json", "--page", "--limit", "100"]:
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
                "usage", "--days", "7", "--json",
                "--wave", "product", "--project", "sessions", "--task", "LOO-265",
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
          "rows": 2,
          "checks": [
            {
              "name": "lineage",
              "status": "ok",
              "detail": "every parent process resolves"
            }
          ]
        }
        """
        let query = RegistryQuery { args, _ in
            #expect(args == ["doctor", "--json"])
            return json
        }

        let report = try await query.doctor()
        #expect(report.rows == 2)
        #expect(report.checks[0].name == "lineage")
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
