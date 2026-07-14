// RegistryQuery decodes the `lf ls/status/runs --json` wire snapshots. The
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
          {"id":"goals","name":"goals","status":"running","paused":false,"goal":"ship the roadmap","repo":"/tmp/repo-a","task_capacity":2,"active_tasks":1,"active_projects":1,"live":true,"endpoint":"127.0.0.1:5678","created_at":null,"parent_wave_id":null},
          {"id":"other","name":"other","status":"idle","paused":false,"goal":"g","repo":"/tmp/repo-b","task_capacity":1,"active_tasks":0,"active_projects":0,"live":false,"endpoint":null,"created_at":null,"parent_wave_id":null}
        ]
        """
        let query = RegistryQuery { args, _ in
            #expect(args == ["ls", "--json"])
            return json
        }

        let waves = try await query.waves(repoPath: "/tmp/repo-a")
        #expect(waves.map(\.id) == ["goals"])
        #expect(waves[0].status == .running)
        #expect(waves[0].repo == "/tmp/repo-a")
    }

    @Test("lf ls can be decoded once for every repo")
    func allWavesDecode() async throws {
        let json = """
        [
          {"id":"goals","name":"goals","status":"running","paused":false,"goal":"ship the roadmap","repo":"/tmp/repo-a","task_capacity":2,"active_tasks":1,"active_projects":1,"live":true,"endpoint":"127.0.0.1:5678","created_at":null,"parent_wave_id":null},
          {"id":"other","name":"other","status":"idle","paused":false,"goal":"g","repo":"/tmp/repo-b","task_capacity":1,"active_tasks":0,"active_projects":0,"live":false,"endpoint":null,"created_at":null,"parent_wave_id":null}
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


    @Test("lf status maps the work hierarchy and attention onto the wave")
    func statusMapsWorkAndAttention() async throws {
        let json = """
        {
          "wave":{"id":"goals","name":"goals","status":"idle","paused":false,"goal":"g","repo":"/tmp/repo-a","task_capacity":1,"active_tasks":1,"active_projects":1,"live":false,"endpoint":null,"created_at":null,"parent_wave_id":null},
          "loop_state":"turning",
          "projects":[{
            "project":{"id":"project-1","slug":"developer-efficiency","name":"Developer efficiency","summary":"Keep flow.","definition":"Remove friction.","krs":[{"text":"Fast loops","holds":false}]},
            "runtime":{"session_id":"ps_1","status":"waiting","reason":"supervised Tasks are active","status_at":"2026-07-06T00:00:00Z","iteration":2,"pending_observations":0,"provider":"codex","process_alive":false},
            "directive":null,
            "next_move":{"owner":"project","reason":"supervised Tasks are active"},
            "tasks":[{
              "task":{"id":"issue-1","identifier":"INF-123","name":"Wire it","description":"","rank":1,"completed":false,"assignee":null},
              "runtime":{"session_id":"ts_1","supervisor":{"kind":"wave","wave_id":"wave-1"},"status":"running","reason":"provider turn is active","status_at":"2026-07-06T00:00:00Z","worktree":"/task-wt","branch":"jack/inf-123","provider":"codex","process_alive":true},
              "directive":null,
              "next_move":{"owner":"task","reason":"provider turn is active"},
              "pull_request":null
            }]
          }],
          "attention":[{"id":"att-1","kind":"interactive","status":"surfaced","title":"needs a human","summary":"review the design","run_id":"run-1","surfaced_at":"2026-07-06T00:00:00Z"}]
        }
        """
        let query = RegistryQuery { args, _ in
            #expect(args == ["status", "goals", "--json"])
            return json
        }

        let result = try await query.status(wave: "goals", waveId: "wave-1", cwd: nil)
        #expect(result.loopState == "turning")
        #expect(result.workMap.projects[0].project.slug == "developer-efficiency")
        #expect(result.workMap.projects[0].runtime?.status == .waiting)
        #expect(result.workMap.projects[0].tasks[0].task.identifier == "INF-123")
        #expect(result.workMap.projects[0].tasks[0].runtime?.supervisor == .wave(id: "wave-1"))
        #expect(result.attention.map(\.id) == ["att-1"])
        #expect(result.attention[0].waveId == "wave-1")
        #expect(result.attention[0].kind == .interactive)
    }

    @Test("lf runs decodes the ledger window")
    func runsDecode() async throws {
        let json = """
        [{"id":"span-1","run_id":"abc","process_id":"span-1","parent_process_id":null,"repo":"/src/loopflow","wave":"goals","label":"gate","status":"ok","started":100,"ended":110,"input_tokens":1000,"output_tokens":200,"cache_read_tokens":800,"cost_usd":0.25,"duration_secs":10.0,"provider":"claude","model":"opus"}]
        """
        let query = RegistryQuery { _, _ in json }

        let runs = try await query.recentRuns()
        #expect(runs.count == 1)
        #expect(runs[0].id == "span-1")
        #expect(runs[0].runId == "abc")
        #expect(runs[0].wave == "goals")
        #expect(runs[0].status == "ok")
        #expect(runs[0].ended == 110)
        #expect(runs[0].cacheReadTokens == 800)
        #expect(runs[0].model == "opus")
    }

    @Test("lf usage --json preserves lineage and unspent spans")
    func spendDecodes() async throws {
        let json = """
        [{"run_id":"abc","process_id":"child","parent_process_id":"parent","seq":7,"node":"run","name":"lf pm show","repo":null,"wave":null,"flow":null,"skill":null,"started_at":100,"ended_at":null,"status":"open","input_tokens":null,"output_tokens":null,"cache_read_tokens":null,"cost_usd":null,"duration_secs":null,"provider":null,"model":null}]
        """
        let query = RegistryQuery { args, _ in
            #expect(args == ["usage", "--json", "--days", "30"])
            return json
        }

        let spans = try await query.spend()
        #expect(spans[0].processId == "child")
        #expect(spans[0].id == "child-7")
        #expect(spans[0].parentProcessId == "parent")
        #expect(spans[0].status == "open")
        #expect(spans[0].endedAt == nil)
        // A span that spent nothing carries nulls, not zeros.
        #expect(spans[0].totalTokens == 0)
        #expect(spans[0].agent == "unattributed")
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

    @Test("PM snapshot exposes only filed open backlog")
    func backlogDecodesOpenTasks() async throws {
        let json = """
        {"wave":"goals","provider":"linear","initiative":"init-1","project":null,"synced_at":1,"projects":[
          {"id":"project-1","slug":"runtime","name":"Runtime","summary":"Run reliably.","definition":"Run reliably.","krs":[],"initiative_ids":["init-1"]}
        ],"items":[
          {"id":"TASK-1","name":"Ship loop","description":"","rank":1,"completed":false,"project":"runtime","assignee":null},
          {"id":"TASK-2","name":"Already done","description":"","rank":2,"completed":true,"project":"runtime","assignee":"user-1"}
        ]}
        """
        let query = RegistryQuery { args, cwd in
            #expect(args == ["pm", "show", "--wave", "goals", "--json", "--no-sync"])
            #expect(cwd == "/tmp/repo")
            return json
        }

        let items = try await query.backlog(wave: "goals", cwd: "/tmp/repo")
        #expect(items.map(\.id) == ["TASK-1"])
        #expect(items[0].project == "runtime")
    }

    @Test("PM snapshot maps projects and KR proof into the wave plan")
    func planDecodesProjects() async throws {
        let json = """
        {"wave":"goals","provider":"linear","initiative":"init-1","project":null,"synced_at":1,"projects":[
          {"id":"project-1","slug":"runtime","name":"Runtime","summary":"Run reliably.","definition":"Run reliably.","krs":[{"text":"Survives restart","holds":true}],"initiative_ids":["init-1"]}
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

    @Test("a failed lf query surfaces as an error")
    func failedQueryThrows() async {
        let query = RegistryQuery { _, _ in throw RegistryQueryError("lf exploded") }
        await #expect(throws: RegistryQueryError.self) {
            _ = try await query.waves(repoPath: "/tmp/repo-a")
        }
    }
}

private actor CallCounter {
    private var count = 0

    var value: Int { count }

    func increment() {
        count += 1
    }
}
