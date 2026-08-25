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
          {"id":"goals","name":"goals","status":"ready","goal":"ship the roadmap","repo":"/tmp/repo-a","active_tasks":1,"active_projects":1,"live":true,"paused":true,"enabled":true,"endpoint":"127.0.0.1:5678","created_at":null,"parent_wave_id":null,"home":{"id":"home_00000000000000000000000000000001","route":"local","created_at":"1970-01-01T00:00:00Z","observed_at":"1970-01-01T00:00:00Z"}},
          {"id":"other","name":"other","status":"ready","goal":"g","repo":"/tmp/repo-b","active_tasks":0,"active_projects":0,"live":false,"paused":false,"enabled":false,"endpoint":null,"created_at":null,"parent_wave_id":null,"home":{"id":"home_00000000000000000000000000000001","route":"local","created_at":"1970-01-01T00:00:00Z","observed_at":"1970-01-01T00:00:00Z"}}
        ]
        """
        let query = RegistryQuery { args, _ in
            #expect(args == ["ls", "--all", "--json"])
            return json
        }

        let waves = try await query.waves(repoPath: "/tmp/repo-a")
        #expect(waves.map(\.id) == ["goals"])
        #expect(waves[0].status == .ready)
        #expect(waves[0].repo == "/tmp/repo-a")
        #expect(waves[0].paused)
        #expect(waves[0].enabled)
    }

    @Test("lf ls can be decoded once for every repo")
    func allWavesDecode() async throws {
        let json = """
        [
          {"id":"goals","name":"goals","status":"ready","goal":"ship the roadmap","repo":"/tmp/repo-a","active_tasks":1,"active_projects":1,"live":true,"paused":true,"enabled":true,"endpoint":"127.0.0.1:5678","created_at":null,"parent_wave_id":null,"home":{"id":"home_00000000000000000000000000000001","route":"local","created_at":"1970-01-01T00:00:00Z","observed_at":"1970-01-01T00:00:00Z"}},
          {"id":"other","name":"other","status":"ready","goal":"g","repo":"/tmp/repo-b","active_tasks":0,"active_projects":0,"live":false,"paused":false,"enabled":false,"endpoint":null,"created_at":null,"parent_wave_id":null,"home":{"id":"home_00000000000000000000000000000001","route":"local","created_at":"1970-01-01T00:00:00Z","observed_at":"1970-01-01T00:00:00Z"}}
        ]
        """
        let counter = CallCounter()
        let query = RegistryQuery { args, _ in
            await counter.increment()
            #expect(args == ["ls", "--all", "--json"])
            return json
        }

        let waves = try await query.allWaves()
        #expect(await counter.value == 1)
        #expect(waves.map(\.id) == ["goals", "other"])
        #expect(waves.map(\.enabled) == [true, false])
    }


    @Test("lf status maps the work hierarchy onto the wave")
    func statusMapsWork() async throws {
        let json = """
        {
          "wave":{"id":"goals","name":"goals","status":"ready","goal":"g","repo":"/tmp/repo-a","active_tasks":1,"active_projects":1,"live":false,"paused":false,"enabled":true,"endpoint":null,"created_at":null,"parent_wave_id":null,"home":{"id":"home_00000000000000000000000000000001","route":"local","created_at":"1970-01-01T00:00:00Z","observed_at":"1970-01-01T00:00:00Z"}},
          "loop_state":"turning",
          "projects":[{
            "project":{"id":"project-1","slug":"developer-efficiency","name":"Developer efficiency","summary":"Keep flow.","definition":"Remove friction.","flows":{"first":"task-design","loop":"slice","finally":"ship"},"krs":[{"text":"Fast loops","holds":false}]},
            "runtime":{"work_id":"project-1","status":"ready","reason":"ready","updated_at":"2026-07-06T00:00:00Z","iteration":2,"pending_observations":0,"provider":"codex","last_failure":null},
            "directive":null,
            "next_move":{"owner":"project","reason":"supervised Tasks are active"},
            "tasks":[{
              "task":{"id":"issue-1","identifier":"INF-123","name":"Wire it","description":"","rank":1,"completed":false,"assignee":null},
              "reference":{"issue_url":"https://linear.app/loopflow/issue/INF-123/wire-it","workspace":{"slug":"wire-it","branch":"jack/inf-123","worktree":"/task-wt"}},
              "runtime":{"work_id":"issue-1","project_id":"project-1","routing_project_id":"project-1","status":"ready","reason":"ready","updated_at":"2026-07-06T00:00:00Z","provider":"codex"},
              "directive":null,
              "next_move":{"owner":"task","reason":"ready"},
              "attention":{"level":"black","reason":"ready","observed_at":"2026-07-06T00:01:00Z","evidence_age_secs":60,"next_owner":"task","actions":{"recommended":"resume","reason":"resume the parked Task"},"pm_completed":false,"work_status":"ready","local_progress":{"state":"observed","unsettled":false,"dirty":false,"authored_commits":false,"recovery_required":false,"reason":null},"active_pr_phase":null},
              "prs":[],
              "active_pr":null
            }]
          }],
          "unavailable_projects":[],
          "runs":{"state":"ok","truncated":false,"items":[{"id":"run_00000000000000000000000000000001","parent_run_id":null,"repo":"/src/loopflow","worktree":"/src/loopflow.task","subjects":[{"selector":"wave:goals","source":"declared"},{"selector":"task:INF-123","source":"declared"}],"skill":"task/pursue","outcome":"completed","started":100,"ended":110,"usage":{"streams":1,"final_streams":1,"gaps":0,"input_tokens":1000,"output_tokens":200,"total_input_tokens":1000,"peak_input_tokens":900,"context_window_tokens":200000,"reasoning_tokens":null,"cache_read_tokens":800,"cache_write_tokens":null,"cost_usd":0.25},"evidence_gaps":0,"harness":"claude","model":"opus","surface":"headless"}]},
          "attention":{"state":"ok","truncated":false,"items":[{"kind":"task","id":"ts_2","subject":"INF-124","owner":"user","reason":"User merge requested","since":"2026-07-06T00:00:00Z","age_secs":7200}]},
          "metric_portfolio":{"metrics":[],"contract_issues":[]},
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
        #expect(result.workMap.projects[0].runtime?.status == .ready)
        #expect(result.workMap.projects[0].tasks[0].task.identifier == "INF-123")
        #expect(result.workMap.projects[0].tasks[0].runtime?.projectId == "project-1")
        #expect(result.workMap.projects[0].tasks[0].reference.issueUrl?.absoluteString.contains("INF-123") == true)
        #expect(result.workMap.projects[0].tasks[0].reference.workspace?.slug == "wire-it")
        #expect(result.workMap.projects[0].tasks[0].reference.workspace?.worktree == "/task-wt")
        #expect(result.workMap.projects[0].tasks[0].reference.workspace?.branch == "jack/inf-123")
        #expect(result.runs.items[0].skill == "task/pursue")
        #expect(result.runs.items[0].usage.inputTokens == 1000)
        #expect(result.attention.items[0].subject == "INF-124")
        #expect(result.attention.items[0].owner == .user)
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

    @Test("lf roadmap requests every repository when no Wave narrows it")
    func roadmapRequestsAllRepositories() async throws {
        let query = RegistryQuery { args, cwd in
            #expect(args == ["roadmap", "--all", "--json"])
            #expect(cwd == nil)
            return #"{"generated_at":"2026-07-15T00:00:00Z","waves":[]}"#
        }

        let result = try await query.roadmap()
        #expect(result.waves.isEmpty)
    }

    @Test("lf activity composes Work filters before the bounded result")
    func workActivityUsesOneFilteredQuery() async throws {
        let json = try String(contentsOf: workActivityFixtureURL(), encoding: .utf8)
        let query = RegistryQuery { args, cwd in
            #expect(args == [
                "activity", "--since", "7d", "--limit", "50",
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

    @Test("Wave Chat history uses the backing-aware DTO")
    func chatHistoryUsesBackingAwareDTO() async throws {
        let query = RegistryQuery { args, cwd in
            #expect(args == [
                "chat", "--history", "--json", "--limit", "12", "--wave", "product",
            ])
            #expect(cwd == "/tmp/repo")
            return #"{"epochs":[],"selected_epoch_id":null,"state":"missing","detail":"No durable Wave Chat history exists yet.","messages":[],"truncated":false}"#
        }

        let snapshot = try await query.chatHistory(
            wave: "product",
            limit: 12,
            cwd: "/tmp/repo"
        )
        #expect(snapshot.state == .missing)
        #expect(snapshot.messages.isEmpty)
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
          "repo":"/tmp/repo","active_tasks":1,"active_projects":1,"live":true,"paused":false,"enabled":true,
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

    @Test("lf start rejects a non-live receipt")
    func startRejectsNonLiveReceipt() async {
        let json = #"""
        [{"id":"wave-1","name":"product","status":"ready","goal":"Ship product",
          "repo":"/tmp/repo","active_tasks":1,"active_projects":1,"live":false,"paused":false,"enabled":true,
          "endpoint":null,"created_at":"2026-07-17T00:00:00Z","parent_wave_id":null,
          "home":{"id":"home_00000000000000000000000000000001","route":"local",
          "created_at":"2026-07-17T00:00:00Z","observed_at":"2026-07-17T00:00:00Z"}}]
        """#
        let query = RegistryQuery { _, _ in json }

        await #expect(throws: RegistryQueryError.self) {
            try await query.start(wave: "product", cwd: "/tmp/repo")
        }
    }

    @Test("lf start surfaces an actionable preflight failure")
    func startSurfacesPreflightFailure() async {
        let query = RegistryQuery { _, _ in
            throw RegistryQueryError("Wave broken failed preflight: invalid chat policy")
        }

        do {
            _ = try await query.start(wave: "broken", cwd: "/tmp/repo")
            Issue.record("expected the preflight failure")
        } catch {
            #expect(error.localizedDescription.contains("failed preflight"))
            #expect(error.localizedDescription.contains("invalid chat policy"))
        }
    }

    @Test("lf pause and resume return the authored turn intent")
    func setWavePausedUsesIntentVerbs() async throws {
        let query = RegistryQuery { args, cwd in
            #expect(cwd == "/tmp/repo")
            switch args {
            case ["pause", "product", "--json"]:
                return #"{"wave":"product","paused":true}"#
            case ["resume", "product", "--json"]:
                return #"{"wave":"product","paused":false}"#
            default:
                throw RegistryQueryError("unexpected argv: \(args)")
            }
        }

        let paused = try await query.setWavePaused(
            wave: "product",
            paused: true,
            cwd: "/tmp/repo"
        )
        #expect(paused == WaveIntentReceipt(wave: "product", paused: true))

        let resumed = try await query.setWavePaused(
            wave: "product",
            paused: false,
            cwd: "/tmp/repo"
        )
        #expect(resumed == WaveIntentReceipt(wave: "product", paused: false))
    }

    /// Unreadable evidence must reach the surface as its reason, never as an
    /// empty list — a broken ledger is not a quiet wave.
    @Test("lf status keeps unavailable evidence unavailable")
    func statusKeepsUnavailableEvidence() async throws {
        let json = """
        {
          "wave":{"id":"goals","name":"goals","status":"ready","goal":"g","repo":"/tmp/repo-a","active_tasks":0,"active_projects":0,"live":false,"paused":false,"enabled":true,"endpoint":null,"created_at":null,"parent_wave_id":null,"home":{"id":"home_00000000000000000000000000000001","route":"local","created_at":"1970-01-01T00:00:00Z","observed_at":"1970-01-01T00:00:00Z"}},
          "loop_state":null,
          "projects":[],
          "unavailable_projects":[],
          "runs":{"state":"unavailable","reason":"run ledger unavailable: disk is gone"},
          "attention":{"state":"ok","truncated":false,"items":[]},
          "metric_portfolio":{"metrics":[],"contract_issues":[]},
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
                return #"{"issue_identifier":"INF-123","task_id":"ts_1","base_commit":"abc","head_commit":"def","files":[{"path":"src/parser.rs","committed":true,"staged":false,"unstaged":true,"untracked":false}]}"#
            case ["task", "diff", "INF-123", "src/parser.rs", "--json"]:
                return #"{"issue_identifier":"INF-123","task_id":"ts_1","path":"src/parser.rs","patch":"@@ -1 +1 @@","binary":false,"truncated":false}"#
            case ["task", "file", "INF-123", "src/parser.rs", "--json"]:
                return #"{"issue_identifier":"INF-123","task_id":"ts_1","path":"src/parser.rs","content":"fn parse() {}\n","binary":false,"size_bytes":14,"truncated":false}"#
            default:
                throw RegistryQueryError("unexpected argv: \(args)")
            }
        }

        let changes = try await query.taskChanges(issue: "INF-123", cwd: "/tmp/repo")
        #expect(changes.taskId == "ts_1")
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

    @Test("User Ask attention prepares and confirms the exact generic Run")
    func userAskAttentionUsesDurableCliHandshake() async throws {
        let attentionJSON = try String(
            contentsOf: askAttentionFixtureURL(),
            encoding: .utf8
        )
        let attention = try JSONDecoder().decode(
            [AskAttentionRecord].self,
            from: Data(attentionJSON.utf8)
        )
        let surfaceData = try Data(contentsOf: askSessionFixtureURL())
        let surface = try JSONDecoder().decode(AskSessionRecord.self, from: surfaceData)
        let ask = try #require(attention.first?.ask)
        let surfaceJSON = String(
            data: try JSONEncoder().encode(surface),
            encoding: .utf8
        )!
        let askJSON = String(
            data: try JSONEncoder().encode(ask),
            encoding: .utf8
        )!
        let query = RegistryQuery { args, cwd in
            #expect(cwd == "/tmp/repo")
            switch args {
            case ["ask", "list", "--user", "--json"]:
                return attentionJSON
            case ["ask", "open", ask.id, "--prepare", "--json"]:
                return surfaceJSON
            case ["ask", "presented", ask.id, surface.runId, "--json"]:
                return askJSON
            default:
                throw RegistryQueryError("unexpected argv: \(args)")
            }
        }

        let listed = try await query.userAskAttention(cwd: "/tmp/repo")
        let opened = try await query.prepareAskOpen(askId: ask.id, cwd: "/tmp/repo")
        let presented = try await query.confirmAskPresented(
            askId: ask.id,
            runId: surface.runId,
            cwd: "/tmp/repo"
        )

        #expect(listed == attention)
        #expect(opened == surface)
        #expect(presented.id == ask.id)
    }

    @Test("lf ps decodes the shared live activity snapshot")
    func activityDecodes() async throws {
        let fixture = try String(contentsOf: activityFixtureURL(), encoding: .utf8)
        let query = RegistryQuery { args, cwd in
            #expect(args == ["ps", "--json"])
            #expect(cwd == nil)
            return fixture
        }

        let snapshot = try await query.processActivity()

        #expect(snapshot.nodes.count == 3)
        #expect(snapshot.nodes.filter { $0.kind == .providerProcess }.count == 2)
        #expect(snapshot.providerProcesses[0].claim == .orphaned)
    }

    @Test("lf usage --json preserves direct Run evidence")
    func usageDecodes() async throws {
        let json = """
        [{"id":"run_00000000000000000000000000000001","parent_run_id":null,"repo":"/src/loopflow","worktree":null,"subjects":[{"selector":"task:LOO-265","source":"declared"}],"skill":"implement","outcome":"completed","started":100,"ended":110,"usage":{"streams":2,"final_streams":1,"gaps":1,"input_tokens":120,"output_tokens":null,"total_input_tokens":120,"peak_input_tokens":100,"context_window_tokens":200000,"reasoning_tokens":null,"cache_read_tokens":80,"cache_write_tokens":null,"cost_usd":0.25},"evidence_gaps":1,"harness":"codex","model":"gpt","surface":"headless"}]
        """
        let query = RegistryQuery { args, _ in
            #expect(args == ["usage", "--days", "30", "--json"])
            return json
        }

        let runs = try await query.usage()
        #expect(runs[0].usage.inputTokens == 120)
        #expect(runs[0].usage.outputTokens == nil)
        #expect(runs[0].usage.finalStreams == 1)
        #expect(runs[0].evidenceGaps == 1)
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

    @Test("PM snapshot maps projects and KR proof into the wave plan")
    func planDecodesProjects() async throws {
        let json = """
        {"wave":"goals","provider":"linear","initiative":"init-1","project":null,"synced_at":1,"projects":[
          {"id":"project-1","slug":"runtime","name":"Runtime","summary":"Run reliably.","definition":"Run reliably.","flows":{"first":null,"loop":null,"finally":null},"krs":[{"text":"Survives restart","holds":true}],"initiative_ids":["init-1"],"team_ids":["team-loo"]}
        ],"items":[{"id":"issue-1","identifier":"LOO-1","url":null,"name":"Wire runtime","description":"","rank":1,"completed":false,"project_id":"project-1","project":"runtime","team_id":"team-loo","assignee":null}]}
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

    @Test("Task invocation refreshes the Wave plan before invocation")
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

private func activityFixtureURL(sourceFile: String = #filePath) -> URL {
    URL(fileURLWithPath: sourceFile)
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .appendingPathComponent("tests/fixtures/dto/activity_snapshot.json")
}

private func workActivityFixtureURL(sourceFile: String = #filePath) -> URL {
    URL(fileURLWithPath: sourceFile)
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .appendingPathComponent("tests/fixtures/dto/work_activity_snapshot.json")
}

private func askAttentionFixtureURL(sourceFile: String = #filePath) -> URL {
    URL(fileURLWithPath: sourceFile)
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .appendingPathComponent("tests/fixtures/dto/ask_attention.json")
}

private func askSessionFixtureURL(sourceFile: String = #filePath) -> URL {
    URL(fileURLWithPath: sourceFile)
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .appendingPathComponent("tests/fixtures/dto/ask_session.json")
}

private actor CallCounter {
    private var count = 0

    var value: Int { count }

    func increment() {
        count += 1
    }
}
