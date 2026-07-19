// RegistryQuery decodes the `lf ls/status/roadmap/runs --json` wire snapshots. The
// runner is injected, so these exercise parsing without spawning `lf`.

import Foundation
import Testing
@testable import Loopflow

@Suite("RegistryQuery")
struct RegistryQueryTests {
    @Test("lf ls decodes and scopes to the repo")
    func wavesDecodeAndScope() async throws {
        let json = """
        [
          {"id":"goals","name":"goals","status":{"running":{"run_id":"run_00000000000000000000000000000001"}},"goal":"ship the roadmap","repo":"/tmp/repo-a","active_tasks":1,"active_projects":1,"live":true,"endpoint":"127.0.0.1:5678","created_at":null,"parent_wave_id":null,"home":{"id":"home_00000000000000000000000000000001","route":"local","created_at":"1970-01-01T00:00:00Z","observed_at":"1970-01-01T00:00:00Z"}},
          {"id":"other","name":"other","status":"ready","goal":"g","repo":"/tmp/repo-b","active_tasks":0,"active_projects":0,"live":false,"endpoint":null,"created_at":null,"parent_wave_id":null,"home":{"id":"home_00000000000000000000000000000001","route":"local","created_at":"1970-01-01T00:00:00Z","observed_at":"1970-01-01T00:00:00Z"}}
        ]
        """
        let query = RegistryQuery { args, _ in
            #expect(args == ["ls", "--json"])
            return json
        }

        let waves = try await query.waves(repoPath: "/tmp/repo-a")
        #expect(waves.map(\.id) == ["goals"])
        #expect(waves[0].status.isRunning)
        #expect(waves[0].repo == "/tmp/repo-a")
    }

    @Test("lf ls can be decoded once for every repo")
    func allWavesDecode() async throws {
        let json = """
        [
          {"id":"goals","name":"goals","status":{"running":{"run_id":"run_00000000000000000000000000000001"}},"goal":"ship the roadmap","repo":"/tmp/repo-a","active_tasks":1,"active_projects":1,"live":true,"endpoint":"127.0.0.1:5678","created_at":null,"parent_wave_id":null,"home":{"id":"home_00000000000000000000000000000001","route":"local","created_at":"1970-01-01T00:00:00Z","observed_at":"1970-01-01T00:00:00Z"}},
          {"id":"other","name":"other","status":"ready","goal":"g","repo":"/tmp/repo-b","active_tasks":0,"active_projects":0,"live":false,"endpoint":null,"created_at":null,"parent_wave_id":null,"home":{"id":"home_00000000000000000000000000000001","route":"local","created_at":"1970-01-01T00:00:00Z","observed_at":"1970-01-01T00:00:00Z"}}
        ]
        """
        let counter = CallCounter()
        let query = RegistryQuery { args, _ in
            await counter.increment()
            #expect(args == ["ls", "--json"])
            return json
        }

        let waves = try await query.allWaves()
        #expect(await counter.value == 1)
        #expect(waves.map(\.id) == ["goals", "other"])
    }


    @Test("lf status maps the work hierarchy onto the wave")
    func statusMapsWork() async throws {
        let json = """
        {
          "wave":{"id":"goals","name":"goals","status":"ready","goal":"g","repo":"/tmp/repo-a","active_tasks":1,"active_projects":1,"live":false,"endpoint":null,"created_at":null,"parent_wave_id":null,"home":{"id":"home_00000000000000000000000000000001","route":"local","created_at":"1970-01-01T00:00:00Z","observed_at":"1970-01-01T00:00:00Z"}},
          "loop_state":"turning",
          "projects":[{
            "project":{"id":"project-1","slug":"developer-efficiency","name":"Developer efficiency","summary":"Keep flow.","definition":"Remove friction.","flows":{"first":"task-design","loop":"slice","finally":"ship"},"krs":[{"text":"Fast loops","holds":false}]},
            "runtime":{"work_id":"project-1","status":{"waiting":{"wait":{"id":"wait_00000000000000000000000000000001","work":{"kind":"project","id":"project-1"},"epoch_id":"epoch_00000000000000000000000000000001","on":{"kind":"input","after":{"epoch_id":"epoch_00000000000000000000000000000001","revision":1}},"created_at":"2026-07-06T00:00:00Z","resolved_at":null}}},"reason":"supervised Tasks are active","updated_at":"2026-07-06T00:00:00Z","iteration":2,"pending_observations":0,"provider":"codex","process_alive":false,"observation":{"category":"needs_input","reason":"supervised Tasks are active","owner":"user","controls":["decide","resume","abandon"],"progress_age_secs":null,"deadline_in_secs":null,"step":"iteration 2"}},
            "directive":null,
            "next_move":{"owner":"project","reason":"supervised Tasks are active"},
            "tasks":[{
              "task":{"id":"issue-1","identifier":"INF-123","name":"Wire it","description":"","rank":1,"completed":false,"assignee":null},
              "reference":{"issue_url":"https://linear.app/loopflow/issue/INF-123/wire-it","workspace":{"slug":"wire-it","branch":"jack/inf-123","worktree":"/task-wt"}},
              "runtime":{"work_id":"issue-1","project_id":"project-1","status":{"running":{"run_id":"run_00000000000000000000000000000002"}},"reason":"provider turn is active","updated_at":"2026-07-06T00:00:00Z","provider":"codex","process_alive":true,"observation":{"category":"working","reason":"provider turn is active","owner":"work","controls":["attach","steer","interrupt","stop"],"progress_age_secs":60,"deadline_in_secs":1740,"step":"iterate"}},
              "directive":null,
              "next_move":{"owner":"task","reason":"provider turn is active"},
              "attention":{"level":"green","reason":"provider turn is active","observed_at":"2026-07-06T00:01:00Z","evidence_age_secs":60,"next_owner":"task","actions":{"recommended":"no_action","reason":"Task body is working"},"pm_completed":false,"work_status":{"running":{"run_id":"run_00000000000000000000000000000002"}},"process":{"state":"observed","alive":true,"reason":null},"local_progress":{"state":"observed","unsettled":false,"dirty":false,"authored_commits":false,"recovery_required":false,"reason":null},"active_pr_phase":null},
              "prs":[],
              "active_pr":null
            }]
          }],
          "runs":{"state":"ok","truncated":false,"items":[{"id":"launch-1","trace_id":"abc","exec_id":"span-1","parent_exec_id":null,"repo":"/src/loopflow","worktree":"/src/loopflow.task","wave":"goals","flow":"task","skill":"task_pursue","status":"ok","started":100,"ended":110,"turns":1,"system_tokens":100,"task_tokens":50,"supplied_context_tokens":150,"input_tokens":1000,"output_tokens":200,"reasoning_tokens":null,"cache_read_tokens":800,"cache_write_tokens":null,"cost_usd":0.25,"duration_secs":10.0,"provider":"claude","model":"opus","surface":"headless","capture_status":"complete"}]},
          "attention":{"state":"ok","truncated":false,"items":[{"kind":"task","id":"ts_2","subject":"INF-124","owner":"review","reason":"PR is open","since":"2026-07-06T00:00:00Z","age_secs":7200}]},
          "home_runtime":{"home":{"id":"home_00000000000000000000000000000001","route":"local","created_at":"1970-01-01T00:00:00Z","observed_at":"1970-01-01T00:00:00Z"},"state":"stopped","reason":"no resident is serving","endpoint":null,"action":{"kind":"start","home_id":"home_00000000000000000000000000000001"}}
        }
        """
        let query = RegistryQuery { args, _ in
            #expect(args == ["status", "goals", "--json"])
            return json
        }

        let result = try await query.status(wave: "goals", cwd: nil)
        #expect(result.wave.id == "goals")
        #expect(result.wave.goal == "g")
        #expect(result.homeRuntime.state == .stopped)
        #expect(result.homeRuntime.action == .start(homeId: "home_00000000000000000000000000000001"))
        #expect(result.loopState == "turning")
        #expect(result.workMap.projects[0].project.slug == "developer-efficiency")
        if case .waiting = result.workMap.projects[0].runtime?.status {} else {
            Issue.record("expected Project Work to be waiting")
        }
        #expect(result.workMap.projects[0].tasks[0].task.identifier == "INF-123")
        #expect(result.workMap.projects[0].tasks[0].runtime?.projectId == "project-1")
        #expect(result.workMap.projects[0].tasks[0].reference.issueUrl?.absoluteString.contains("INF-123") == true)
        #expect(result.workMap.projects[0].tasks[0].reference.workspace?.slug == "wire-it")
        #expect(result.workMap.projects[0].tasks[0].reference.workspace?.worktree == "/task-wt")
        #expect(result.workMap.projects[0].tasks[0].reference.workspace?.branch == "jack/inf-123")
        #expect(result.runs.items[0].skill == "task_pursue")
        #expect(result.runs.items[0].suppliedContextTokens == 150)
        #expect(result.attention.items[0].subject == "INF-124")
        #expect(result.attention.items[0].owner == .review)
        #expect(result.attention.items[0].ageSeconds == 7200)
    }

    @Test("lf roadmap is one optionally scoped machine query")
    func roadmapUsesOneMachineQuery() async throws {
        let query = RegistryQuery { args, cwd in
            #expect(args == ["roadmap", "--wave", "product", "--json"])
            #expect(cwd == nil)
            return #"{"generated_at":"2026-07-15T00:00:00Z","waves":[]}"#
        }

        let result = try await query.roadmap(wave: "product")
        #expect(result.generatedAt == "2026-07-15T00:00:00Z")
        #expect(result.waves.isEmpty)
    }

    @Test("Wave Chat history is a bounded local query")
    func chatHistoryUsesLocalJournalQuery() async throws {
        let query = RegistryQuery { args, cwd in
            #expect(args == [
                "chat", "--history", "--json", "--limit", "12", "--wave", "product",
            ])
            #expect(cwd == "/tmp/repo")
            return #"{"state":"missing","detail":"No durable Wave Chat history exists yet.","turns":[],"truncated":false}"#
        }

        let snapshot = try await query.chatHistory(
            wave: "product",
            limit: 12,
            cwd: "/tmp/repo"
        )
        #expect(snapshot.state == .missing)
        #expect(snapshot.turns.isEmpty)
        #expect(!snapshot.truncated)
    }

    @Test("lf home probe decodes the state and the one contextual action")
    func homeProbeDecodesStateAndAction() async throws {
        let json = #"""
        {"home":{"id":"home_00000000000000000000000000000001","route":"local","created_at":"1970-01-01T00:00:00Z","observed_at":"1970-01-01T00:00:00Z"},
         "state":"stopped","reason":"reachable, no resident","endpoint":null,
         "action":{"kind":"start","home_id":"home_00000000000000000000000000000001"}}
        """#
        let query = RegistryQuery { args, cwd in
            #expect(args == ["home", "probe", "product", "--json"])
            #expect(cwd == "/tmp/repo")
            return json
        }

        let runtime = try await query.homeProbe(wave: "product", cwd: "/tmp/repo")
        #expect(runtime.state == .stopped)
        #expect(runtime.endpoint == nil)
        #expect(runtime.action == .start(homeId: "home_00000000000000000000000000000001"))
    }

    @Test("lf start returns the existing Wave status contract")
    func startReturnsWaveStatus() async throws {
        let json = #"""
        [{"id":"wave-1","name":"product","status":"ready","goal":"Ship product",
          "repo":"/tmp/repo","active_tasks":1,"active_projects":1,"live":true,
          "endpoint":"127.0.0.1:7777","created_at":"2026-07-17T00:00:00Z",
          "parent_wave_id":null,"home":{"id":"home_00000000000000000000000000000001",
          "route":"local","created_at":"2026-07-17T00:00:00Z",
          "observed_at":"2026-07-17T00:00:00Z"}}]
        """#
        let query = RegistryQuery { args, cwd in
            #expect(args == ["start", "product", "--json"])
            #expect(cwd == "/tmp/repo")
            return json
        }

        let result = try await query.start(wave: "product", cwd: "/tmp/repo")
        #expect(result[0].live)
        #expect(result[0].endpoint == "127.0.0.1:7777")
        #expect(result[0].home.id == "home_00000000000000000000000000000001")
    }

    /// Unreadable evidence must reach the surface as its reason, never as an
    /// empty list — a broken ledger is not a quiet wave.
    @Test("lf status keeps unavailable evidence unavailable")
    func statusKeepsUnavailableEvidence() async throws {
        let json = """
        {
          "wave":{"id":"goals","name":"goals","status":"ready","goal":"g","repo":"/tmp/repo-a","active_tasks":0,"active_projects":0,"live":false,"endpoint":null,"created_at":null,"parent_wave_id":null,"home":{"id":"home_00000000000000000000000000000001","route":"local","created_at":"1970-01-01T00:00:00Z","observed_at":"1970-01-01T00:00:00Z"}},
          "loop_state":null,
          "projects":[],
          "runs":{"state":"unavailable","reason":"run ledger unavailable: disk is gone"},
          "attention":{"state":"ok","truncated":false,"items":[]},
          "home_runtime":{"home":{"id":"home_00000000000000000000000000000001","route":"local","created_at":"1970-01-01T00:00:00Z","observed_at":"1970-01-01T00:00:00Z"},"state":"stopped","reason":"no resident is serving","endpoint":null,"action":{"kind":"start","home_id":"home_00000000000000000000000000000001"}}
        }
        """
        let query = RegistryQuery { _, _ in json }

        let result = try await query.status(wave: "goals", cwd: nil)
        #expect(result.runs.unavailableReason == "run ledger unavailable: disk is gone")
        #expect(result.runs.items.isEmpty)
        #expect(result.attention.unavailableReason == nil)
        #expect(result.attention.items.isEmpty)
    }

    @Test("Task workspace queries preserve paths and binary/truncation evidence")
    func taskWorkspaceQueriesDecode() async throws {
        let query = RegistryQuery { args, cwd in
            #expect(cwd == "/tmp/repo")
            switch args {
            case ["task", "changes", "INF-123", "--json"]:
                return #"{"issue_identifier":"INF-123","session_id":"ts_1","base_commit":"abc","head_commit":"def","files":[{"path":"src/parser.rs","committed":true,"staged":false,"unstaged":true,"untracked":false}]}"#
            case ["task", "diff", "INF-123", "src/parser.rs", "--json"]:
                return #"{"issue_identifier":"INF-123","session_id":"ts_1","path":"src/parser.rs","patch":"@@ -1 +1 @@","binary":false,"truncated":false}"#
            case ["task", "file", "INF-123", "src/parser.rs", "--json"]:
                return #"{"issue_identifier":"INF-123","session_id":"ts_1","path":"src/parser.rs","content":"fn parse() {}\n","binary":false,"size_bytes":14,"truncated":false}"#
            default:
                throw RegistryQueryError("unexpected argv: \(args)")
            }
        }

        let changes = try await query.taskChanges(issue: "INF-123", cwd: "/tmp/repo")
        #expect(changes.sessionId == "ts_1")
        #expect(changes.files[0].path == "src/parser.rs")
        #expect(changes.files[0].committed && changes.files[0].unstaged)

        let diff = try await query.taskDiff(
            issue: "INF-123",
            path: "src/parser.rs",
            cwd: "/tmp/repo"
        )
        #expect(diff.patch == "@@ -1 +1 @@")
        #expect(!diff.binary && !diff.truncated)

        let file = try await query.taskFile(
            issue: "INF-123",
            path: "src/parser.rs",
            cwd: "/tmp/repo"
        )
        #expect(file.content == "fn parse() {}\n")
        #expect(file.sizeBytes == 14)
    }

    @Test("lf runs decodes the ledger window")
    func runsDecode() async throws {
        let json = """
        [{"id":"launch-1","trace_id":"abc","exec_id":"span-1","parent_exec_id":null,"repo":"/src/loopflow","worktree":"/src/loopflow","wave":"goals","project":"auditability","task":"W2-122","flow":"build","skill":"gate","status":"ok","started":100,"ended":110,"turns":1,"system_tokens":100,"task_tokens":50,"supplied_context_tokens":150,"input_tokens":1000,"output_tokens":200,"reasoning_tokens":null,"cache_read_tokens":800,"cache_write_tokens":null,"cost_usd":0.25,"duration_secs":10.0,"provider":"claude","model":"opus","surface":"headless","capture_status":"complete"},
         {"id":"launch-2","trace_id":"def","exec_id":"span-2","parent_exec_id":null,"repo":"/src/loopflow","worktree":"/src/loopflow","wave":"goals","project":null,"task":null,"flow":null,"skill":"debug","status":"running","started":120,"ended":null,"turns":0,"system_tokens":0,"task_tokens":0,"supplied_context_tokens":0,"input_tokens":null,"output_tokens":null,"reasoning_tokens":null,"cache_read_tokens":null,"cache_write_tokens":null,"cost_usd":null,"duration_secs":null,"provider":"claude","model":null,"surface":"headless","capture_status":"pending"}]
        """
        let query = RegistryQuery { _, _ in json }

        let runs = try await query.recentRuns()
        #expect(runs.count == 2)
        #expect(runs[0].id == "launch-1")
        #expect(runs[0].traceId == "abc")
        #expect(runs[0].wave == "goals")
        #expect(runs[0].status == "ok")
        #expect(runs[0].ended == 110)
        #expect(runs[0].cacheReadTokens == 800)
        #expect(runs[0].suppliedContextTokens == 150)
        #expect(runs[0].model == "opus")
        // The drill foreign key: a run declares the roadmap Project/Task it owns,
        // or nil when it was launched outside a Task Session.
        #expect(runs[0].project == "auditability")
        #expect(runs[0].task == "W2-122")
        #expect(runs[1].project == nil)
        #expect(runs[1].task == nil)
    }

    @Test("lf usage --json decodes one additive Turn row")
    func spendDecodes() async throws {
        let json = """
        [{"turn_id":"turn-1","launch_id":"launch-1","trace_id":"abc","exec_id":"child","repo":"/src/loopflow","wave":null,"flow":"build","skill":"gate","provider":"claude","model":"opus","at":100,"input_tokens":1000,"output_tokens":200,"cache_read_tokens":800,"cost_usd":0.25}]
        """
        let query = RegistryQuery { args, _ in
            #expect(args == ["usage", "--json", "--days", "30"])
            return json
        }

        let turns = try await query.spend()
        #expect(turns[0].id == "turn-1")
        #expect(turns[0].launchId == "launch-1")
        #expect(turns[0].traceId == "abc")
        #expect(turns[0].execId == "child")
        #expect(turns[0].totalTokens == 1200)
        #expect(turns[0].agent == "claude:opus")
    }

    @Test("lf doctor decodes every check")
    func doctorDecodes() async throws {
        let json = """
        {"rows":2,"checks":[{"name":"lineage","status":"ok","detail":"every parent process resolves"}]}
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

    @Test("Context Lab sends one filter query and decodes one atomic snapshot")
    func contextLabUsesOneAggregateQuery() async throws {
        let fixture = try String(contentsOf: contextLabFixtureURL(), encoding: .utf8)
        let query = RegistryQuery { args, cwd in
            #expect(cwd == nil)
            #expect(args == [
                "context", "--json", "--started-after", "100", "--started-before", "200",
                "--repo", "/src/loopflow", "--wave", "intelligence",
                "--project", "context", "--task", "W2-71",
                "--skill", "implement", "--provider", "codex", "--model", "gpt-5",
                "--surface", "headless", "--outcome", "completed",
                "--capture-state", "complete",
                "--steered-only", "--current-revision-only",
            ])
            return fixture
        }
        let selection = SessionSetQuery(
            repoPaths: ["/src/loopflow"],
            startedAfter: 100,
            startedBefore: 200,
            waves: ["intelligence"],
            projects: ["context"],
            tasks: ["W2-71"],
            flows: [],
            skills: ["implement"],
            providers: ["codex"],
            models: ["gpt-5"],
            surfaces: ["headless"],
            outcomes: [.completed],
            captureStates: [.complete],
            steeredOnly: true,
            currentRevisionOnly: true
        )

        let snapshot = try await query.contextLab(selection)

        #expect(snapshot.query == selection)
        #expect(snapshot.aggregateRoot.attributedTokens == 800)
        #expect(snapshot.evidence[0].representatives[0].address.launchId == "launch-1")
    }

    @Test("Trace bodies load only through the explicit content query")
    func traceContentUsesExactAddress() async throws {
        let address = TraceAddress(runId: "run-1", launchId: "launch-1", turnId: "turn-1")
        let query = RegistryQuery { args, cwd in
            #expect(cwd == nil)
            #expect(args == [
                "trace", "run-1", "--json", "--content",
                "--launch", "launch-1", "--turn", "turn-1",
            ])
            return """
            {"address":{"run_id":"run-1","launch_id":"launch-1","turn_id":"turn-1"},"system_prompt":{"path":null,"content":null,"unavailable_reason":"turn has no system prompt"},"task_prompt":{"path":"/trace/task.txt","content":"exact task","unavailable_reason":null},"conversation":{"path":"/trace/events.jsonl","content":null,"unavailable_reason":"missing"}}
            """
        }

        let trace = try await query.traceContent(address)

        #expect(trace.address == address)
        #expect(trace.taskPrompt.content == "exact task")
        #expect(trace.systemPrompt.unavailableReason == "turn has no system prompt")
        #expect(trace.conversation.content == nil)
    }

    @Test("PM snapshot maps projects and KR proof into the wave plan")
    func planDecodesProjects() async throws {
        let json = """
        {"wave":"goals","provider":"linear","initiative":"init-1","project":null,"synced_at":1,"projects":[
          {"id":"project-1","slug":"runtime","name":"Runtime","summary":"Run reliably.","definition":"Run reliably.","flows":{"first":null,"loop":null,"finally":null},"krs":[{"text":"Survives restart","holds":true}],"initiative_ids":["init-1"]}
        ],"items":[]}
        """
        let query = RegistryQuery { args, cwd in
            #expect(args == ["pm", "show", "--wave", "goals", "--json", "--no-sync"])
            #expect(cwd == "/tmp/repo")
            return json
        }

        let plan = try await query.plan(wave: "goals", objective: "Ship it.", cwd: "/tmp/repo")
        #expect(plan.objective == "Ship it.")
        #expect(plan.projects[0].id == "runtime")
        #expect(plan.projects[0].definition == "Run reliably.")
        #expect(plan.projects[0].krs[0].proof == .holds)
    }

    @Test("Task launch refreshes the Wave plan before launch")
    func planCanSyncProjects() async throws {
        let query = RegistryQuery { args, cwd in
            #expect(args == ["pm", "show", "--wave", "goals", "--json", "--sync"])
            #expect(cwd == "/tmp/repo")
            return """
            {"wave":"goals","provider":"linear","initiative":"init-1","project":null,"synced_at":1,"projects":[],"items":[]}
            """
        }

        _ = try await query.plan(
            wave: "goals",
            objective: "",
            cwd: "/tmp/repo",
            sync: true
        )
    }

    @Test("a failed lf query surfaces as an error")
    func failedQueryThrows() async {
        let query = RegistryQuery { _, _ in throw RegistryQueryError("lf exploded") }
        await #expect(throws: RegistryQueryError.self) {
            _ = try await query.waves(repoPath: "/tmp/repo-a")
        }
    }
}

private func contextLabFixtureURL(sourceFile: String = #filePath) -> URL {
    URL(fileURLWithPath: sourceFile)
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .appendingPathComponent("tests/fixtures/dto/context_lab_snapshot.json")
}

private actor CallCounter {
    private var count = 0

    var value: Int { count }

    func increment() {
        count += 1
    }
}
